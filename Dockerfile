#
# Stage 'build-ephyr' builds Ephyr for the final stage.
#

# https://github.com/jrottenberg/ffmpeg/blob/master/docker-images/4.3/centos7/Dockerfile
FROM jrottenberg/ffmpeg:4.3-centos7 AS build-ephyr


# Install Rust.
WORKDIR /tmp/rust/

ENV RUSTUP_HOME=/tmp/rust/rustup \
    CARGO_HOME=/tmp/rust/cargo \
    PATH=/tmp/rust/cargo/bin:$PATH

RUN curl -sLO https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init \
 && chmod +x rustup-init \
 && ./rustup-init -y --no-modify-path --profile minimal \
                  --default-toolchain stable \
 && chmod -R a+w $RUSTUP_HOME $CARGO_HOME \
 && rustup --version \
 && cargo --version \
 && rustc --version


# Install build dependencies.
RUN yum --enablerepo=extras install -y epel-release \
 && yum --enablerepo=epel install -y automake gcc libtool make \
                                     openssl-devel


# Build Ephyr.
WORKDIR /tmp/ephyr/

# First, build all the dependencies to cache them in a separate Docker layer and
# avoid recompilation each time project sources are changed.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/ && touch src/lib.rs
RUN cargo build --lib --release

# Now, build the project itself.
RUN rm -rf ./target/release/.fingerprint/ephyr-*
COPY src/ ./src/
RUN cargo build --bin ephyr --release




#
# Stage 'build-srs' prepares SRS distribution for the final stage.
#

# https://github.com/ossrs/srs-docker/blob/v3/Dockerfile
FROM ossrs/srs:3 AS build-srs




#
# Stage 'runtime' creates final Docker image to use in runtime.
#

# https://github.com/jrottenberg/ffmpeg/blob/master/docker-images/4.3/centos7/Dockerfile
FROM jrottenberg/ffmpeg:4.3-centos7 AS dist

COPY --from=build-ephyr /tmp/ephyr/target/release/ephyr /usr/local/bin/ephyr

COPY --from=build-srs /usr/local/srs/ /usr/local/srs/

WORKDIR /usr/local/srs/
ENTRYPOINT  ["/usr/local/srs/objs/srs"]
CMD ["-c", "conf/srs.conf"]
