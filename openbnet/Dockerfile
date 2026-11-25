FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY assets assets
COPY migrations migrations
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 openbnet && install -d -o openbnet /data
COPY --from=build /src/target/release/openbnet /usr/local/bin/openbnet
USER openbnet
ENV OPENBNET_DATA=/data
VOLUME /data
EXPOSE 6112/tcp 6112/udp
ENTRYPOINT ["openbnet"]
CMD ["serve"]
