###############################
# Common defaults/definitions #
###############################

OS_NAME := $(shell uname -s)




############
# Commands #
############

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
#	make publish

play:
	ffplay -rtmp_live 1 rtmp://127.0.0.1:1935/stream/some


# Publish raw local camera RTMP stream to re-streaming application.
#
# Usage:
#	make publish

publish:
ifeq ($(OS_NAME),Darwin)
	ffmpeg -f avfoundation -video_device_index 0 -audio_device_index 0 -i '' \
	       -f flv rtmp://127.0.0.1:11935/stream/some
else
	$(error "'publish' command is not implemented for your OS")
endif


# Run development environment.
#
# Usage:
#	make up

up: down
	docker-compose up -d
	cargo run -- push -h 127.0.0.1 -a stream -s some -t some




##################
# .PHONY section #
##################

.PHONY: down play publish up
