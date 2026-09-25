# Security Policy

The Agent Guidance team takes security and responsible disclosure seriously. We appreciate your efforts to responsibly disclose vulnerabilities to protect our community and users.

---

## Supported Versions

We provide security updates and patches for the following versions of **Agent Guidance**:

| Version | Supported          |
| :---    | :---               |
| 1.8.x   | :white_check_mark: |
| 1.7.x   | :x:                |
| < 1.7.0 | :x:                |

We strongly recommend always upgrading to the latest release (`agent-guidance --upgrade`) to ensure you have the latest security protections and safety patches.

---

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues, pull requests, or discussions.**

Instead, please report security issues through one of the following private channels:

### 1. GitHub Private Vulnerability Reporting (Preferred)
You can report a vulnerability directly on GitHub by navigating to:
**[Security Tab $\rightarrow$ Report a vulnerability](https://github.com/JunMystery/Agent-Guidance-Rust/security/advisories/new)**

This opens a private advisory workspace between you and the maintainers to discuss, verify, and resolve the issue confidentially before public disclosure.

### 2. Direct Security Email
If you prefer email or cannot use GitHub Advisories, email:
**[contact@junmystery.com](mailto:contact@junmystery.com)**

Please include:
- A clear description of the vulnerability and attack vector.
- Step-by-step instructions or minimal Proof of Concept (PoC) to reproduce the issue.
- The affected component (e.g. MCP JSON-RPC router, GraphRAG engine, Token Compressor, Remote ML Worker).
- Affected version(s) and operating system.
- Potential impact or blast radius.

---

## Response & Disclosure Process

1. **Acknowledgment**: We aim to acknowledge receipt of your vulnerability report within **48 hours**.
2. **Assessment & Triage**: We will investigate, verify the issue, and provide an initial assessment within **5 business days**.
3. **Fix Development**: If confirmed, we will develop and test a patch in a private repository branch.
4. **Coordinated Disclosure**: Once a patch is prepared, we will publish a patched release, credit the reporter (unless anonymity is requested), and publish a GitHub Security Advisory detailing the mitigation.

---

## Security Best Practices for Users

- **Bearer Token Protection**: When binding the remote server worker to `0.0.0.0`, always configure a strong `--api-key` to prevent unauthorized network access.
- **Stdio Isolation**: Keep your IDE client MCP configurations scoped to trusted environments.
- **Workflow State Enforcement**: Do not bypass `workflow_gate` stage checks or 300 LOC limits with unverified scripts.
