ARG RUST_IMAGE=rust:1.99.1-trixie
FROM ${RUST_IMAGE} AS build

RUN apt-get update \
    && apt-get install --yes --no-install-recommends clang cmake nasm ninja-build python3 \
    && { \
        dpkg-query --show --showformat='${Package}\t${Version}\n' clang cmake nasm ninja-build python3; \
        cc --version; \
        c++ --version; \
        ld --version; \
        cmake --version; \
        ninja --version; \
        nasm --version; \
    } > /container-build-tools.txt \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /source
COPY . .
RUN python3 tools/prepare_native.py \
    && cargo build --locked --release \
    && strip target/release/imglean

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:3ee00291cf386ec52a552e6a858d555debd74776eb4bfa53fa3df2cd6677515e

ARG VERSION=dev
ARG SOURCE_COMMIT=unknown
ARG SOURCE_REPOSITORY=unknown/unknown
LABEL org.opencontainers.image.title="ImgLean" \
      org.opencontainers.image.description="Focused offline same-format image optimization CLI" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${SOURCE_COMMIT}" \
      org.opencontainers.image.source="https://github.com/${SOURCE_REPOSITORY}"

COPY --from=build /source/target/release/imglean /usr/local/bin/imglean
COPY LICENSE.md THIRD_PARTY_NOTICES.md /licenses/imglean/
COPY --from=build /container-build-tools.txt /licenses/imglean/BUILD_TOOLS.txt

USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/imglean"]
