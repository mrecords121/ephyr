###############################
# Common defaults/definitions #
###############################

# Checks two given strings for equality.
eq = $(if $(or $(1),$(2)),$(and $(findstring $(1),$(2)),\
                                $(findstring $(2),$(1))),1)

OS_NAME := $(shell uname -s)




###########
# Aliases #
###########

fmt: cargo.fmt


lint: cargo.lint




############
# Commands #
############

# Apply audio filters to background volume with ZeroMQ.
#
# Usage:
#	make audio [volume=<volume-rate>] [delay=<milliseconds>]

audio:
ifneq ($(volume),)
	docker run --rm --network=host --entrypoint sh tyranron/srs:3 -c \
		'echo "volume@y volume $(volume)" \
		 | zmqsend -b tcp://127.0.0.1:11235'
endif
ifneq ($(delay),)
	docker run --rm --network=host --entrypoint sh tyranron/srs:3 -c \
		'echo "adelay@x reinit delays=$(delay)|all=1" \
		 | zmqsend -b tcp://127.0.0.1:11235'
endif


# List to STDOUT available audio/video devices with FFmpeg.
#
# Usage:
#	make devices.list

devices.list:
ifeq ($(OS_NAME),Darwin)
	-ffmpeg -f avfoundation -list_devices true -i ''
else
	$(error "'devices.list' command is not implemented for your OS")
endif


# Stop running development environment and remove all related Docker containers.
#
# Usage:
#	make down

cargo-run-pid = $(word 1,$(shell ps -ax | grep -v grep \
                                        | grep 'target/' \
                                        | grep '/rtmp-mixing-poc'))

down:
ifneq ($(cargo-run-pid),)
	kill $(cargo-run-pid)
endif
	docker-compose down --rmi=local -v


# Play re-streamed RTMP stream from Nginx.
#
# Usage:
#	make play [from=(youtube|edge)]

play:
	ffplay -rtmp_live 1 \
		rtmp://127.0.0.1:1935$(if $(call eq,$(from),edge),0,)/live/ru-en$(if $(call eq,$(from),edge),_ff,)


# Publish raw local camera RTMP stream to re-streaming application.
#
# Usage:
#	make publish

publish:
ifeq ($(OS_NAME),Darwin)
	ffmpeg -f avfoundation -video_device_index 0 -audio_device_index 0 -i '' \
	       -f flv rtmp://127.0.0.1:19351/ingest/ru-en
else
	$(error "'publish' command is not implemented for your OS")
endif


# Run development environment.
#
# Usage:
#	make up [background=(no|yes)]

up: down
	docker-compose up \
		$(if $(call eq,$(background),yes),-d,--abort-on-container-exit)
#	cargo run -- push -h 127.0.0.1 -a stream -s some -t some




##################
# Cargo commands #
##################

# Resolve Cargo project dependencies.
#
# Usage:
#	make cargo [cmd=(fetch|<cargo-cmd>)]
#	           [background=(no|yes)]
#	           [dockerized=(no|yes)]

cargo-cmd = $(if $(call eq,$(cmd),),fetch,$(cmd))

cargo:
ifeq ($(dockerized),yes)
ifeq ($(background),yes)
	-@docker stop cargo-cmd
	-@docker rm cargo-cmd
endif
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           --name=cargo-cmd $(if $(call eq,$(background),yes),-d,) \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		rust:$(RUST_VER) \
			make cargo cmd='$(cargo-cmd)' dockerized=no background=no
else
	cargo $(cargo-cmd) $(if $(call eq,$(background),yes),&,)
endif


# Format Rust sources with rustfmt.
#
# Usage:
#	make cargo.fmt [check=(no|yes)]
#	               [dockerized=(no|yes)]

cargo.fmt:
ifeq ($(dockerized),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		instrumentisto/rust:$(RUST_NIGHTLY_VER) \
			make cargo.fmt check='$(check)' dockerized=no
else
	cargo +nightly fmt --all $(if $(call eq,$(check),yes),-- --check,)
endif


# Lint Rust sources with clippy.
#
# Usage:
#	make cargo.lint [dockerized=(no|yes)]

cargo.lint:
ifeq ($(dockerized),yes)
	docker run --rm --network=host -v "$(PWD)":/app -w /app \
	           -v "$(abspath $(CARGO_HOME))/registry":/usr/local/cargo/registry\
		rust:$(RUST_VER) \
			make cargo.lint dockerized=no
else
	cargo clippy --all -- -D clippy::pedantic -D warnings
endif




##################
# .PHONY section #
##################

.PHONY: audio devices.list down fmt lint play publish up \
        cargo cargo.fmt cargo.lint
