# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

ERL_ROOT_DIR := $(shell erl -noshell -eval 'io:format("~s", [code:root_dir()])' -s init stop)
ERL_INCLUDE_DIR := $(ERL_ROOT_DIR)/usr/include
ERL_LIB_DIR := $(ERL_ROOT_DIR)/usr/lib

CC ?= gcc
CFLAGS := -I$(ERL_INCLUDE_DIR) -O2 -Wall -fPIC
LDFLAGS := -L$(ERL_LIB_DIR) -lerl_interface -lei -shared

ifeq ($(OS),Windows_NT)
	LIB_EXT = .dll
	LDFLAGS += -lws2_32
else
	LIB_EXT = .so
	ifeq ($(shell uname),Darwin)
		LDFLAGS += -undefined dynamic_lookup -flat_namespace
	endif
endif

PRIV_DIR = ../priv
TARGET = $(PRIV_DIR)/disk_space$(LIB_EXT)
SRC = disk_space.c

all: $(TARGET)

$(TARGET): $(SRC) | $(PRIV_DIR)
	$(CC) $(CFLAGS) $(SRC) $(LDFLAGS) -o $(TARGET)

$(PRIV_DIR):
	mkdir -p $(PRIV_DIR)

clean:
	rm -f $(TARGET)

.PHONY: all clean