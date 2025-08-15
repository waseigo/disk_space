# This file includes code generated or modified by xAI's Grok 3 
# over multiple rounds of prompting for reviews and improvements that were
# suggested by Grok 3, GPT-5 and Gemini 2.5 Flash.

# Determine Erlang root directory and version
ERL := $(shell which erl 2>/dev/null || echo "not found")
ERL_ROOT_DIR := $(shell $(ERL) -noshell -eval 'io:format("~s", [code:root_dir()])' -s init stop 2>/dev/null || echo "not found")
ERL_VERSION := $(shell $(ERL) -noshell -eval 'io:format("~s", [erlang:system_info(version)])' -s init stop 2>/dev/null || echo "not found")
ifneq ($(ERL_ROOT_DIR),not found)
    ERL_INCLUDE_DIR := $(ERL_ROOT_DIR)/erts-$(ERL_VERSION)/include
else
    $(error Erlang not found or failed to detect ERL_ROOT_DIR)
endif

# Compiler and flags
CC ?= gcc
CFLAGS := -I$(ERL_INCLUDE_DIR) -O2 -Wall -Werror -fPIC
LDFLAGS :=

# Source and target
SRC := disk_space.c
PRIV_DIR := ../priv
UNAME_S := $(shell uname -s 2>/dev/null || echo "not found")
LIB_EXT := .so
MKDIR := mkdir -p
RM := rm -f

# Platform-specific settings
ifeq ($(UNAME_S),Darwin)
    CC := $(shell command -v clang >/dev/null && echo clang || echo gcc)
    LDFLAGS += -dynamiclib -undefined dynamic_lookup -arch arm64 -arch x86_64 -lErlNifSmp
else ifeq ($(OS),Windows_NT)
    LIB_EXT := .dll
    ifeq ($(CC),cl)
        CFLAGS := /I$(ERL_INCLUDE_DIR) /O2 /W3 /WX /D_CRT_SECURE_NO_WARNINGS
        LDFLAGS := /DLL /link /LIBPATH:$(ERL_ROOT_DIR)/erts-$(ERL_VERSION)/lib
        MKDIR := md
        RM := del /Q
    else
        LDFLAGS += -shared -lErlNifSmp
    endif
else
    LDFLAGS += -shared -lErlNifSmp -lrt
endif

TARGET := $(PRIV_DIR)/disk_space$(LIB_EXT)

# Default target
all: $(TARGET)

# Create priv directory
$(PRIV_DIR):
	-$(MKDIR) $(PRIV_DIR)

# Compile and link
$(TARGET): $(SRC) | $(PRIV_DIR)
	$(CC) $(CFLAGS) $(SRC) $(LDFLAGS) -o $(TARGET)

# Clean up
clean:
	-$(RM) $(TARGET)

.PHONY: all clean
