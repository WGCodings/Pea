EXE = Pea
VER = 7.0

ifeq ($(OS),Windows_NT)
    SUFFIX := .exe
    PLATFORM := windows
else
    SUFFIX :=
    PLATFORM := linux
endif

ARCH := x86_64

NAME     := $(EXE)$(SUFFIX)
STANDARD := $(EXE)-$(VER)-$(PLATFORM)-$(ARCH)$(SUFFIX)
AVX2     := $(EXE)-$(VER)-$(PLATFORM)-$(ARCH)-avx2-bmi2$(SUFFIX)

rule:
	cargo clean
	cargo rustc --release --bin $(EXE) -- -C target-cpu=native --emit link=$(NAME)

release:
	cargo rustc --release --bin $(EXE) -- --emit link=$(STANDARD)
	cargo rustc --release --bin $(EXE) -- -C target-cpu=x86-64-v2 -C target-feature=+avx2,+bmi2 --emit link=$(AVX2)