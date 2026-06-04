DEVICE ?= Scarlett 18i8 USB
INPUT_CHANNEL ?= 1
OUTPUT_CHANNELS ?= 1,2
SAMPLE_RATE ?= 48000
PERIOD_SIZE ?= 32

build:
	cargo build --release

standalone: build
	target/release/voxbox-standalone --device '$(DEVICE)' \
		--input-channel $(INPUT_CHANNEL) --output-channels $(OUTPUT_CHANNELS) \
		--sample-rate $(SAMPLE_RATE) --period-size $(PERIOD_SIZE)

standalone-with-ir: build
	target/release/voxbox-standalone --device '$(DEVICE)' \
		--input-channel $(INPUT_CHANNEL) --output-channels $(OUTPUT_CHANNELS) \
		--sample-rate $(SAMPLE_RATE) --period-size $(PERIOD_SIZE) --ir

devices: build
	target/release/voxbox-standalone --list-devices
