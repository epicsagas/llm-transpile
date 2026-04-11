# Data Retention Policy

Governs how user data is stored, retained, and deleted.

## Personal Data

All data classified as personal under applicable regulations.

- Personal data must be encrypted at rest using AES-256.
- Personal data must not be retained beyond 90 days after account deletion.
- Access to personal data must be logged with user identity and timestamp.
- Personal data should be pseudonymised where processing does not require identification.
- Analysts may access aggregated, anonymised data without additional approval.

## Audit Logs

System and access audit trails.

- Audit logs must be retained for a minimum of 12 months.
- Audit logs must not be modified or deleted by application processes.
- Audit logs should be stored in an append-only storage system.
- Logs may be archived to cold storage after 90 days.

### Log Integrity

- Log entries must include a cryptographic hash of the previous entry.
- Log integrity must be verified on a weekly schedule.

## Deletion Requests

- User deletion requests must be processed within 30 days.
- Deletion must propagate to all downstream data stores.
- Backup copies must be purged within 7 days of the primary deletion.
- Users should receive confirmation upon completion of deletion.

The legal retention period for financial transaction logs is TBD pending counsel review.
