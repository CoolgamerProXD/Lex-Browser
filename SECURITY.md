# Security Policy

Lex is experimental and **must not yet be used for security-sensitive browsing**. It currently has no network stack, content sandbox, origin enforcement, or renderer isolation.

Report vulnerabilities privately through GitHub's security advisory feature rather than a public issue. Do not include credentials, cookies, tokens, private browsing information, or other personal data in reports or logs.

Security invariants include validated TLS by default, no HTTPS downgrade, explicit permissions for privileged operations, origin-based access control, no secrets in logs, and an untrusted-content renderer boundary before broad web compatibility.
