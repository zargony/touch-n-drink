use crate::Updater;
use crate::reader::LineReader;
use alloc::string::{String, ToString};
use alloc::{format, vec};
use derive_more::{Display, From};
use embassy_time::{Duration, Instant, with_deadline, with_timeout};
use embedded_io_async::BufRead;
use embedded_nal_async::{Dns, TcpConnect};
use embedded_storage::Storage;
use log::{debug, info, warn};
use reqwless::client::{HttpClient, HttpConnection};
use reqwless::request::Method;
use reqwless::response::{Response, StatusCode};
use sha2::{Digest, Sha256};

/// URL of GitHub repository to download releases from
const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Prefix to strip from the release tag to get the release version
const RELEASE_TAG_VERSION_PREFIX: &str = "firmware-";

/// OTA update name of checksum file to download
const CHECKSUMS_FILENAME: &str = "SHA256SUMS";

/// How long to wait for a server response
const TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait to finish streaming a server's response
const FETCH_TIMEOUT: Duration = Duration::from_secs(180);

/// Maximum size of response headers from server
/// Unfortunately, github.com download responses contain large headers and need >4k :-(
const MAX_RESPONSE_HEADER_SIZE: usize = 5120;

/// Maximum number of redirects to follow
const MAX_REDIRECTS: usize = 5;

/// OTA update error
#[derive(Debug, Display, From)]
#[must_use]
pub enum Error {
    /// Updater error (actual flashing / partition handling)
    // FIXME: using embedded String instead Updater::Error to avoid inconvenient generics
    #[display("Updater: {_0}")]
    Updater(String),
    /// Network error
    #[from]
    #[display("Network: {_0}")]
    Network(reqwless::Error),
    /// Received malformed redirect
    #[display("Malformed redirect")]
    MalformedRedirect,
    /// Too many redirects
    #[display("Too many redirects")]
    TooManyRedirects,
    /// Request failed
    #[display("Request failed ({})", _0.0)]
    RequestFailed(StatusCode),
    /// Unable to query latest release
    #[display("Unable to query latest release")]
    UnableToQueryLatestRelease,
    /// Unable to fetch checksum
    #[display("Unable to fetch checksum")]
    UnableToFetchChecksum,
    /// Flashing failed
    #[display("Flashing failed")]
    FlashingFailed,
    /// Checksum mismatch
    #[display("Checksum mismatch")]
    ChecksumMismatch,
    /// Timeout waiting for response
    #[from(embassy_time::TimeoutError)]
    #[display("Timeout")]
    Timeout,
}

/// OTA updater
#[must_use]
pub struct Ota<'ota, 'http, T: TcpConnect, D: Dns> {
    http: &'ota mut HttpClient<'http, T, D>,
}

impl<'ota, 'http, T: TcpConnect, D: Dns> Ota<'ota, 'http, T, D> {
    /// Create new OTA updater
    pub fn new(http: &'ota mut HttpClient<'http, T, D>) -> Self {
        Self { http }
    }
}

impl<T: TcpConnect, D: Dns> Ota<'_, '_, T, D> {
    /// Check for latest release and update if a new release is available
    ///
    /// # Errors
    ///
    /// An error will be returned if checking or updating fails.
    pub async fn check_and_update<U: Updater>(
        &mut self,
        updater: &mut U,
    ) -> Result<Option<String>, Error> {
        if let Some(new_version) = self.check().await? {
            self.update(updater, &new_version).await?;
            Ok(Some(new_version))
        } else {
            Ok(None)
        }
    }

    /// Get latest release version and return it when it's a different version than the currently
    /// running version. Returns None if the latest release version is already running.
    ///
    /// # Errors
    ///
    /// An error will be returned if checking for a new version fails.
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

    /// Update to given release version
    ///
    /// # Errors
    ///
    /// An error will be returned if updating to the new version fails.
    pub async fn update<U: Updater>(
        &mut self,
        updater: &mut U,
        version: &str,
    ) -> Result<(), Error> {
        info!("Ota: Updating to release v{version}...");

        // Get checksum of release version
        let expected_checksum = self
            .get_release_checksum(U::FIRMWARE_VARIANT, version)
            .await?;

        // Download and write firmware to flash region and calculate actual checksum
        let res = {
            // Get flash region to write new firmware to
            let mut region = updater
                .region()
                .map_err(|e| Error::Updater(e.to_string()))?;

            self.download_and_flash_release(U::FIRMWARE_VARIANT, version, &mut region)
                .await
        };
        match res {
            // Checksum of downloaded firmare matches listed release checksum: continue
            Ok(checksum) if checksum == expected_checksum => (),
            // Checksum does not match: abort update
            Ok(checksum) => {
                warn!(
                    "Ota: Checksum mismatch! Expected {}, got {}",
                    const_hex::const_encode::<32, false>(&expected_checksum).as_str(),
                    const_hex::const_encode::<32, false>(&checksum).as_str(),
                );
                updater.cancel();
                return Err(Error::ChecksumMismatch);
            }
            // Error downloading or flashing: abort update
            Err(err) => {
                updater.cancel();
                return Err(err);
            }
        }

        // Commit firmware update so bootloader will use it on next restart
        updater
            .commit()
            .map_err(|e| Error::Updater(e.to_string()))?;

        info!("Ota: Successfully updated to v{version}. Restart required.");
        Ok(())
    }

    /// Get latest release version
    ///
    /// # Errors
    ///
    /// An error will be returned if the latest release version could not be determined.
    pub async fn get_latest_release_version(&mut self) -> Result<String, Error> {
        let tag = self.get_latest_release_tag().await?;
        if let Some(version) = tag.strip_prefix(RELEASE_TAG_VERSION_PREFIX) {
            Ok(version.to_string())
        } else {
            Err(Error::UnableToQueryLatestRelease)
        }
    }
}

impl<T: TcpConnect, D: Dns> Ota<'_, '_, T, D> {
    /// Compose image filename
    fn image_filename(variant: &str) -> String {
        format!("touch-n-drink-{variant}.bin")
    }

    /// Download release, store it in given flash region and return its calculated checksum
    async fn download_and_flash_release<F: Storage>(
        &mut self,
        variant: &str,
        version: &str,
        flash_region: &mut F,
    ) -> Result<[u8; 32], Error> {
        let url = format!(
            "{REPOSITORY_URL}/releases/download/{RELEASE_TAG_VERSION_PREFIX}{version}/{}",
            Self::image_filename(variant),
        );
        self.http_get_fn(url, true, async |response| {
            with_timeout(FETCH_TIMEOUT, async {
                let mut reader = response.body().reader();
                let mut offset = 0;
                let mut hasher = Sha256::new();
                while let data = reader.fill_buf().await?
                    && !data.is_empty()
                {
                    flash_region
                        .write(offset, data)
                        // TODO: Retaining the exact error would be nice, but leads to inconvenient generics
                        .map_err(|_e| Error::FlashingFailed)?;
                    hasher.update(data);
                    let len = data.len();
                    reader.consume(len);
                    offset += u32::try_from(len).unwrap(); // safe to unwrap because read buffer size < u32::MAX
                    debug!("Ota: Flashed {offset} bytes");
                }
                Ok(hasher.finalize().into())
            })
            .await?
        })
        .await
    }

    /// Get checksum of given release version
    async fn get_release_checksum(
        &mut self,
        variant: &str,
        version: &str,
    ) -> Result<[u8; 32], Error> {
        let url = format!(
            "{REPOSITORY_URL}/releases/download/{RELEASE_TAG_VERSION_PREFIX}{version}/{CHECKSUMS_FILENAME}"
        );
        self.http_get_fn(url, true, async |response| {
            with_timeout(TIMEOUT, async {
                let image_filename = Self::image_filename(variant);
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
                        Some(filename) if filename == image_filename.as_bytes() => {
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
        self.http_get_fn(url, false, async |response| {
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
    async fn http_get_fn<F, R>(
        &mut self,
        mut url: String,
        follow_redirects: bool,
        f: F,
    ) -> Result<R, Error>
    where
        F: AsyncFnOnce(Response<'_, '_, HttpConnection<'_, T::Connection<'_>>>) -> Result<R, Error>,
    {
        let deadline = Instant::now() + TIMEOUT;
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE].into_boxed_slice();

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
