# Authentication Policy

This document defines authentication requirements for the platform.

## User Login

Handles credential validation and session establishment.

- Users must provide a valid username and password.
- Passwords must not be stored in plain text.
- Failed login attempts must be logged with timestamp and IP address.
- Sessions should expire after 30 minutes of inactivity.
- Users may enable two-factor authentication for additional security.

## Token Management

Manages access and refresh token lifecycle.

- Access tokens must be signed using RS256.
- Refresh tokens must not be transmitted over unencrypted connections.
- Tokens should be rotated after each successful refresh.
- Token expiry should be set to 15 minutes for access tokens.

## Password Policy

- Passwords must be at least 12 characters long.
- Passwords must contain at least one uppercase letter, one digit, and one special character.
- Passwords should not be reused within the last 10 cycles.
- Users may use a password manager to generate credentials.

The exact brute-force threshold is TBD.
