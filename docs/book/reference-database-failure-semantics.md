# Reference: Database Failure Semantics

Use this page when you want one compact lookup table for the current
database-oriented failure surfaces without opening each protocol page
individually.

## Query/Error Family

| Protocol | Entry | Typical signal | Failure mode | Failure detail |
| --- | --- | --- | --- | --- |
| `mysql` | `error` | `ERR` | `semantic_error` | `protocol_error` |
| `postgres` | `error` | `ErrorResponse` | `semantic_error` | `protocol_error` |

## Auth-Denied Family

| Protocol | Entry | Typical signal | Failure mode | Failure detail |
| --- | --- | --- | --- | --- |
| `mysql` | `auth-denied` | `ERR` | `server_denied` | `access_denied` |
| `postgres` | `auth-denied` | `ErrorResponse` | `server_denied` | `access_denied` |

## Directory Write Family

| Protocol | Entry | Typical signal | Failure mode | Failure detail |
| --- | --- | --- | --- | --- |
| `ldap` | `denied` | `modifyResponse` | `server_denied` | `access_denied` |
| `ldap` | `constraint` | `modifyResponse` | `semantic_error` | `protocol_constraint_violation` |

## Reading Order

If you are validating the current database and directory failure spine, the
shortest useful reading order is:

1. [docs/book/reference-mysql-error-surface.md](docs/book/reference-mysql-error-surface.md)
2. [docs/book/reference-postgres-error-surface.md](docs/book/reference-postgres-error-surface.md)
3. [docs/book/reference-ldap-write-surface.md](docs/book/reference-ldap-write-surface.md)
4. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
