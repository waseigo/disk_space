# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

ERL_ROOT_DIR := $(shell erl -noshell -eval 'io:format("~s", [code:root_dir()])' -s init stop)
ERL_INCLUDE_DIR := $(ERL_ROOT_DIR)/usr/include

CC ?= gcc
CFLAGS := -I$(ERL_INCLUDE_DIR) -O2 -Wall -fPIC
LDFLAGS :=

SRC = disk_space.c
PRIV_DIR = ../priv

UNAME_S := $(shell uname)
LIB_EXT = .so

ifeq ($(UNAME_S),Darwin)    
    CFLAGS += -arch arm64
    LDFLAGS += -dynamiclib -undefined dynamic_lookup -arch arm64
else ifeq ($(OS),Windows_NT)
    LIB_EXT = .dll
    LDFLAGS += -shared -lws2_32
else
    LDFLAGS += -shared
endif

TARGET = $(PRIV_DIR)/disk_space$(LIB_EXT)

all: $(TARGET)

$(TARGET): $(SRC) | $(PRIV_DIR)
	$(CC) $(CFLAGS) $(SRC) $(LDFLAGS) -o $(TARGET)

$(PRIV_DIR):
	mkdir -p $(PRIV_DIR)

clean:
	rm -f $(TARGET)

.PHONY: all clean

