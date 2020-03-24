###############################
# Common defaults/definitions #
###############################

# Checks two given strings for equality.
eq = $(if $(or $(1),$(2)),$(and $(findstring $(1),$(2)),\
                                $(findstring $(2),$(1))),1)

OS_NAME := $(shell uname -s)




############
# Commands #
############

# Apply audion filters to background volume with ZeroMQ.
#
# Usage:
#	make audio volume=<volume-rate>

audio:
	echo Parsed_volume_1 volume $(volume) | zmqsend -b tcp://127.0.0.1:11235


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
		rtmp://127.0.0.1:1935$(if $(call eq,$(from),edge),0,)/stream/some$(if $(call eq,$(from),edge),_ff,)


# Publish raw local camera RTMP stream to re-streaming application.
#
# Usage:
#	make publish

publish:
ifeq ($(OS_NAME),Darwin)
	ffmpeg -f avfoundation -video_device_index 0 -audio_device_index 0 -i '' \
	       -f flv rtmp://127.0.0.1:19351/stream/some
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
# .PHONY section #
##################

.PHONY: audio devices.list down play publish up
