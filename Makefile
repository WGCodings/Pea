EXE = Pea

ifeq ($(OS),Windows_NT)
    SUFFIX := .exe
    PLATFORM := windows
else
    SUFFIX :=
    PLATFORM := linux
endif


NAME     := $(EXE)$(SUFFIX)
AVX		 :=  $(EXE)-AVX2$(SUFFIX)

rule:
	cargo clean
	cargo rustc --release --bin Pea -- -C target-cpu=native --emit link=$(NAME)

release:
	cargo rustc --release --bin Pea -- -C target-cpu=native -C target-feature=+avx2,+bmi2 --emit link=$(NAME)