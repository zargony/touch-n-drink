# Touch 'n Drink Firmware Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- Provide app image (`.bin`) and factory image (`.factory.bin`, includes bootloader and partition table)
- Fix CI building image with stock partition table instead our customized one

## 1.0.0 - 2025-03-06

- Installed in club room
- Fix "Buffer too small" NFC error with some cards

## 0.3.0 - 2025-01-22

- Allow to choose from multiple articles to purchase
- Automatically refresh article and user information once a day
- Event tracking using Mixpanel for usage analytics

## 0.2.0 - 2024-11-27

- Show random greetings to user

## 0.1.0 - 2024-10-30

- Showcased on general meeting
- Simple flow: authorize user by NFC, select number of drinks, confirm, purchase
- Purchases are created on Vereinsflieger
- Fetch article price from Vereinsflieger
- Fetch list of authorized users and their NFC uid from Vereinsflieger
- Static configuration stored on device flash
