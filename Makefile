###############################
# Common defaults/definitions #
###############################

# Checks two given strings for equality.
eq = $(if $(or $(1),$(2)),$(and $(findstring $(1),$(2)),\
                                $(findstring $(2),$(1))),1)

OS_NAME := $(shell uname -s)




######################
# Project parameters #
######################

IMAGE_NAME ?= $(strip $(shell grep 'IMAGE_NAME=' .env | cut -d '=' -f2))
IMAGE_TAG ?= $(strip $(shell grep 'IMAGE_TAG=' .env | cut -d '=' -f2))




###########
# Aliases #
###########

down: docker.down


fmt: cargo.fmt


image: docker.image


lint: cargo.lint


up: docker.up




########################
# Interaction commands #
########################

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


# Play mixed RTMP stream from Origin SRS.
#
# Usage:
#	make play [stream=(output/musicify_mic|<app>/<stream>)]

play-stream = $(if $(call eq,$(stream),),output/musicify_mic,$(stream))

play:
	ffplay -rtmp_live 1 rtmp://127.0.0.1:1935/$(play-stream)


# Publish raw local camera RTMP stream to Origin SRS.
#
# Usage:
#	make publish [stream=(input/mic|<app>/<stream>)]

publish-stream = $(if $(call eq,$(stream),),input/trance,$(stream))

publish:
ifeq ($(OS_NAME),Darwin)
	ffmpeg -f avfoundation -video_device_index 0 -audio_device_index 0 -i '' \
	       -f flv rtmp://127.0.0.1:1935/$(publish-stream)
else
	$(error "'publish' command is not implemented for your OS")
endif


# Tune audio filters on-fly for mixed RTMP stream.
#
# Usage:
#	make tune volume=<volume-rate> [track=(music|original)]

tune-track = $(if $(call eq,$(track),),music,$(track))
tune-volume-port = $(if $(call eq,$(tune-track),music),60002,60001)

tune:
ifneq ($(volume),)
	docker run --rm --network=host --entrypoint sh \
		$(IMAGE_NAME):$(IMAGE_TAG) -c \
			'echo "volume@$(tune-track) volume $(volume)" \
			 | zmqsend -b tcp://127.0.0.1:$(tune-volume-port)'
endif




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
	cargo clippy --all -- -D clippy::pedantic -D warnings \
		-A clippy::enum_glob_use
endif




###################
# Docker commands #
###################

# Stop project in Docker Compose development environment
# and remove all related containers.
#
# Usage:
#	make docker.down

docker.down:
	docker-compose down --rmi=local -v


# Build project Docker image.
#
# Usage:
#	make docker.image [tag=($(IMAGE_TAG)|<tag>)]
#	                  [no-cache=(no|yes)]

docker.image:
	docker build --network=host --force-rm \
		$(if $(call eq,$(no-cache),yes),\
			--no-cache --pull,) \
		-t $(IMAGE_NAME):$(if $(call eq,$(tag),),$(IMAGE_TAG),$(tag)) ./


# Run project in Docker Compose development environment.
#
# Usage:
#	make docker.up [rebuild=(no|yes)] [background=(no|yes)]

docker.up: docker.down
ifeq ($(rebuild),yes)
	@make docker.image
endif
	docker-compose up \
		$(if $(call eq,$(background),yes),-d,--abort-on-container-exit)




##################
# .PHONY section #
##################

.PHONY: down fmt image lint up \
        play publish tune \
        cargo cargo.fmt cargo.lint \
        devices.list \
        docker.down docker.image docker.up
