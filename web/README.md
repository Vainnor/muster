# Muster website

Server-rendered with [Topcoat](https://github.com/tokio-rs/topcoat) 0.10.0. The pages send no
JavaScript, set no cookies and load nothing from other origins; `src/security.rs` sets a strict
CSP and the other security headers on every response.

| Path | Source |
| --- | --- |
| `/` | `src/app.rs` (layout and home; the download button label comes from `src/download.rs`) |
| `/features`, `/privacy`, `/terms`, `/license` | `src/app/*.rs` |
| `/theme.css` | `src/theme_css.rs`: the light and dark presets from `muster-ui` |

The privacy policy and terms are drafts, not legal advice. The contact address is the single
placeholder `CONTACT_EMAIL` in `src/site.rs`; replace it before going live.

## Run locally

```sh
cargo install topcoat-cli --version 0.10.0 --locked
topcoat dev -p muster-web          # rebuilds and reloads on change
# or, without the dev server:
topcoat asset bundle -p muster-web && PORT=8080 cargo run -p muster-web
```

Format with `cargo fmt --all && topcoat fmt web/src`.

## Container

```sh
docker build -f web/Dockerfile -t muster-web .   # from the repository root
docker run --rm -p 127.0.0.1:8080:8080 --read-only --cap-drop ALL muster-web
```

Multi-stage: the build stage compiles the release binary and bundles its assets; the final
stage is distroless (`cc-debian12:nonroot`), runs as uid 65532 and has no shell. CI builds the
image in `.github/workflows/web.yml` on every change to `web/` but does not push or deploy it;
the manifests below expect it at `ghcr.io/vainnor/muster-web`.

## Kubernetes

`deploy/` holds example manifests: a `restricted` Pod Security namespace, a two-replica
Deployment with a read-only root filesystem, a Service, an Ingress for `muster.vainnor.com`
(cert-manager TLS), and a NetworkPolicy that denies all egress. Edit the marked values
(ingress class, issuer, ingress namespace label, image tag), then `kubectl apply -k web/deploy`.
