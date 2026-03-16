use crate::reader::LineReader;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::{format, vec};
use core::fmt;
use embassy_time::{Duration, Instant, with_deadline, with_timeout};
use embedded_io_async::BufRead;
use embedded_nal_async::{Dns, TcpConnect};
use embedded_storage::Storage;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_bootloader_esp_idf::partitions::{self, FlashRegion};
use log::{debug, info, warn};
use reqwless::client::{HttpClient, HttpConnection, TlsConfig, TlsVerify};
use reqwless::request::Method;
use reqwless::response::{Response, StatusCode};
use sha2::{Digest, Sha256};

/// URL of GitHub repository to download releases from
const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Prefix to strip from the release tag to get the release version
const RELEASE_TAG_VERSION_PREFIX: &str = "firmware-";

/// OTA update name of image file to download
const IMAGE_FILENAME: &str = concat!(env!("CARGO_PKG_NAME"), "-esp32c3.bin");

/// OTA update name of checksum file to download
const CHECKSUMS_FILENAME: &str = "SHA256SUMS";

/// How long to wait for a server response
const TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait to finish streaming a server's response
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum number of redirects to follow
const MAX_REDIRECTS: usize = 5;

/// Maximum size of response headers from server.
/// Unfortunately, github.com download responses contain large headers and need >4k :-(
const MAX_RESPONSE_HEADER_SIZE: usize = 5120;

/// HTTP (TLS) read buffer size. Needs to fit an encrypted TLS record. Actual size depends on
/// server configuration. Maximum allowed value for a TLS record is 16640.
const READ_BUFFER_SIZE: usize = 16640;

/// HTTP (TLS) write buffer size
const WRITE_BUFFER_SIZE: usize = 2048;

/// OTA update error
#[derive(Debug)]
pub enum Error {
    /// Invalid partition setup
    InvalidPartitionSetup(partitions::Error),
    /// Network error
    Network(reqwless::Error),
    /// Received malformed redirect
    MalformedRedirect,
    /// Too many redirects
    TooManyRedirects,
    /// Request failed
    RequestFailed(StatusCode),
    /// Unable to query latest release
    UnableToQueryLatestRelease,
    /// Unable to fetch checksum
    UnableToFetchChecksum,
    /// Flashing failed
    FlashingFailed(partitions::Error),
    /// Checksum mismatch
    ChecksumMismatch,
    /// Timeout waiting for response
    Timeout,
}

impl From<reqwless::Error> for Error {
    fn from(err: reqwless::Error) -> Self {
        Self::Network(err)
    }
}

impl From<embassy_time::TimeoutError> for Error {
    fn from(_err: embassy_time::TimeoutError) -> Self {
        Self::Timeout
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPartitionSetup(err) => write!(f, "Invalid partition setup: {err}"),
            Self::Network(err) => write!(f, "Network: {err}"),
            Self::MalformedRedirect => write!(f, "Malformed redirect"),
            Self::TooManyRedirects => write!(f, "Too many redirects"),
            Self::RequestFailed(status) => write!(f, "Request failed ({})", status.0),
            Self::UnableToQueryLatestRelease => {
                write!(f, "Unable to query latest release")
            }
            Self::UnableToFetchChecksum => write!(f, "Unable to fetch checksum"),
            Self::FlashingFailed(err) => write!(f, "Flashing failed: {err}"),
            Self::ChecksumMismatch => write!(f, "Checksum mismatch"),
            Self::Timeout => write!(f, "Timeout"),
        }
    }
}

/// OTA updater resources
pub struct Resources {
    read_buffer: Box<[u8]>,
    write_buffer: Box<[u8]>,
}

impl Resources {
    /// Create new OTA updater resources
    pub fn new() -> Self {
        Self {
            read_buffer: vec![0; READ_BUFFER_SIZE].into_boxed_slice(),
            write_buffer: vec![0; WRITE_BUFFER_SIZE].into_boxed_slice(),
        }
    }
}

/// OTA updater
pub struct Ota<'a, T: TcpConnect, D: Dns> {
    http: HttpClient<'a, T, D>,
}

impl<'a, T: TcpConnect, D: Dns> Ota<'a, T, D> {
    /// Create new OTA updater
    pub fn new(tcp: &'a T, dns: &'a D, seed: u64, resources: &'a mut Resources) -> Self {
        // FIXME: reqwless with embedded-tls can't verify TLS certificates (though pinning is
        // supported). This is bad since it makes communication vulnerable to MITM attacks.
        let tls_config = TlsConfig::new(
            seed,
            &mut resources.read_buffer,
            &mut resources.write_buffer,
            TlsVerify::None,
        );
        let http = HttpClient::new_with_tls(tcp, dns, tls_config);

        Self { http }
    }

    /// Check for latest release and update if a new release is available
    #[expect(dead_code)]
    pub async fn check_and_update<F: Storage>(
        &mut self,
        flash: &mut F,
    ) -> Result<Option<String>, Error> {
        if let Some(new_version) = self.check().await? {
            self.update(&new_version, flash).await?;
            Ok(Some(new_version))
        } else {
            Ok(None)
        }
    }

    /// Update to given release version
    pub async fn update<F: Storage>(&mut self, version: &str, flash: &mut F) -> Result<(), Error> {
        info!("Ota: Updating to release v{version}...");

        let expected_checksum = self.get_release_checksum(version).await?;

        let mut buf = [0; partitions::PARTITION_TABLE_MAX_LEN];
        let mut updater = OtaUpdater::new(flash, &mut buf).map_err(Error::InvalidPartitionSetup)?;
        let current_subtype = updater
            .selected_partition()
            .map_err(Error::InvalidPartitionSetup)?;
        let (mut region, subtype) = updater
            .next_partition()
            .map_err(Error::InvalidPartitionSetup)?;
        debug!("Ota: Current partition: {current_subtype:?}, flashing to: {subtype:?}");

        // TODO: Erase partition first to get rid of any remains?

        let checksum = self
            .download_and_flash_release(version, &mut region)
            .await?;
        if checksum != expected_checksum {
            warn!(
                "Ota: Checksum mismatch! Expected {}, got {}",
                const_hex::const_encode::<32, false>(&expected_checksum).as_str(),
                const_hex::const_encode::<32, false>(&checksum).as_str(),
            );
            // TODO: Erase partition because of potentially bad app image
            return Err(Error::ChecksumMismatch);
        }

        // Mark the written partition active so bootloader will use it on next restart
        debug!("Ota: Checksum ok, activating partition");
        updater
            .activate_next_partition()
            .map_err(Error::FlashingFailed)?;
        // FIXME: Explicitly set the state of the new image slot to valid because
        //  - the bootloader has no rollback support
        //  - esp-bootloader-esp-ids 0.4.0 retains the old state of whatever was in the
        //    ota data slot before, which seems wrong as we just flashed a new image
        let _ = updater.set_current_ota_state(OtaImageState::Valid);

        info!("Ota: Successfully updated to v{version}. Restart required.");
        Ok(())
    }

    /// Get latest release version and return it when it's a different version than the currently
    /// running version. Returns None if the latest release version is already running.
    pub async fn check(&mut self) -> Result<Option<String>, Error> {
        let current = env!("CARGO_PKG_VERSION");
        let latest = self.get_latest_release_version().await?;
        // For now, this is intentionally not using semver comparison, so that it simply updates
        // to whatever version is set as the latest release.
        if latest == current {
            info!("Ota: Already running latest release v{current}");
            Ok(None)
        } else {
            info!("Ota: New release available: v{latest}, currently running v{current}");
            Ok(Some(latest))
        }
    }

    /// Get latest release version
    pub async fn get_latest_release_version(&mut self) -> Result<String, Error> {
        let tag = self.get_latest_release_tag().await?;
        if let Some(version) = tag.strip_prefix(RELEASE_TAG_VERSION_PREFIX) {
            Ok(version.to_string())
        } else {
            Err(Error::UnableToQueryLatestRelease)
        }
    }
}

impl<T: TcpConnect, D: Dns> Ota<'_, T, D> {
    /// Download release, store it in given flash region and return its calculated checksum
    async fn download_and_flash_release<F: Storage>(
        &mut self,
        version: &str,
        flash: &mut FlashRegion<'_, F>,
    ) -> Result<[u8; 32], Error> {
        let url = format!(
            "{REPOSITORY_URL}/releases/download/{RELEASE_TAG_VERSION_PREFIX}{version}/{IMAGE_FILENAME}"
        );
        self.send_request(url, true, async |response| {
            with_timeout(FETCH_TIMEOUT, async {
                let mut reader = response.body().reader();
                let mut offset = 0;
                let mut hasher = Sha256::new();
                while let data = reader.fill_buf().await?
                    && !data.is_empty()
                {
                    flash.write(offset, data).map_err(Error::FlashingFailed)?;
                    hasher.update(data);
                    let len = data.len();
                    reader.consume(len);
                    offset += u32::try_from(len).unwrap(); // safe to unwrap because READ_BUFFER_SIZE < u32::MAX
                    debug!("Ota: Flashed {offset} bytes");
                }
                Ok(hasher.finalize().into())
            })
            .await?
        })
        .await
    }

    /// Get checksum of given release version
    async fn get_release_checksum(&mut self, version: &str) -> Result<[u8; 32], Error> {
        let url = format!(
            "{REPOSITORY_URL}/releases/download/{RELEASE_TAG_VERSION_PREFIX}{version}/{CHECKSUMS_FILENAME}"
        );
        self.send_request(url, true, async |response| {
            with_timeout(TIMEOUT, async {
                let mut reader: LineReader<_> = LineReader::new(response.body().reader());
                while let Some(line) = reader
                    .next()
                    .await
                    .map_err(|_| Error::UnableToFetchChecksum)?
                {
                    // Read through sha256sums file. Each line has 64 characters hex digest, 2 spaces, filename, newline.
                    let mut elements = line.split(|b| *b == b' ');
                    // 64 characters hex digest
                    let digest: [u8; 32] = match elements.next() {
                        Some(hex) => match const_hex::decode_to_array(hex) {
                            Ok(digest) => digest,
                            Err(_err) => continue,
                        },
                        None => continue,
                    };
                    // 2 spaces (empty element)
                    match elements.next() {
                        Some(&[]) => (),
                        Some(_) | None => continue,
                    }
                    // Filename
                    match elements.next() {
                        Some(filename) if filename == IMAGE_FILENAME.as_bytes() => {
                            debug!(
                                "Ota: Release v{version} checksum {}: {}",
                                String::from_utf8_lossy(filename),
                                const_hex::const_encode::<32, false>(&digest).as_str()
                            );
                            return Ok(digest);
                        }
                        // Some(_filename) => continue,
                        // None => continue,
                        _ => (),
                    }
                }
                // EOF
                Err(Error::UnableToFetchChecksum)
            })
            .await?
        })
        .await
    }

    /// Get latest release tag
    // TODO: This simply queries the `releases/latest` URL for the latest release. A more
    // sophisticated approach would be to query `api.github.com/repos/<owner>/<repo>/releases`
    // and select the most appropriate release from it.
    async fn get_latest_release_tag(&mut self) -> Result<String, Error> {
        let url = format!("{REPOSITORY_URL}/releases/latest");
        self.send_request(url, false, async |response| {
            if !response.status.is_redirection() {
                return Err(Error::UnableToQueryLatestRelease);
            }
            let location = response
                .headers()
                .find_map(|(k, v)| (k.eq_ignore_ascii_case("location")).then_some(v))
                .and_then(|v| str::from_utf8(v).ok())
                .ok_or(Error::MalformedRedirect)?;
            let mut path_components = location.rsplit('/');
            let tag = path_components
                .next()
                .ok_or(Error::UnableToQueryLatestRelease)?
                .to_string();
            if !matches!(path_components.next(), Some("tag"))
                || !matches!(path_components.next(), Some("releases"))
            {
                return Err(Error::UnableToQueryLatestRelease);
            }
            debug!("Ota: Latest release tag: {tag}");
            Ok(tag)
        })
        .await
    }

    /// Send HTTP GET request, optionally follow redirects and call closure with response object
    async fn send_request<F, R>(
        &mut self,
        mut url: String,
        follow_redirects: bool,
        f: F,
    ) -> Result<R, Error>
    where
        F: AsyncFnOnce(Response<'_, '_, HttpConnection<'_, T::Connection<'_>>>) -> Result<R, Error>,
    {
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE].into_boxed_slice();
        let deadline = Instant::now() + TIMEOUT;

        for _ in 0..MAX_REDIRECTS {
            debug!("Ota: GET {url}");
            let mut request =
                with_deadline(deadline, self.http.request(Method::GET, &url)).await??;
            let response = with_deadline(deadline, request.send(&mut rx_buf)).await??;
            debug!("Ota: HTTP status {}", response.status.0);

            // If response is redirect and should follow redirects, parse location header and continue
            if response.status.is_redirection() && follow_redirects {
                let location = response
                    .headers()
                    .find_map(|(k, v)| (k.eq_ignore_ascii_case("location")).then_some(v))
                    .map(|v| String::from_utf8_lossy(v).into_owned())
                    .ok_or(Error::MalformedRedirect)?;
                drop(request);
                url = location;

            // For any non-error status code, call the provided closure to process the response
            } else if response.status.is_informational()
                || response.status.is_successful()
                || response.status.is_redirection()
            {
                return f(response).await;

            // Any other status code indicates a failed request
            } else {
                return Err(Error::RequestFailed(response.status));
            }
        }

        Err(Error::TooManyRedirects)
    }
}
