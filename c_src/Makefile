# SPDX-FileCopyrightText: 2025 Isaak Tsalicoglou <isaak@overbring.com>
# SPDX-License-Identifier: Apache-2.0

ERL_INCLUDE_DIR := $(shell erl -noshell -eval 'io:format("~s",[code:root_dir()])' -s init stop)/usr/include

ifeq ($(OS),Windows_NT)
	LIB_EXT = .dll
else
	LIB_EXT = .so
endif

PRIV_DIR = ../priv
TARGET = $(PRIV_DIR)/disk_space$(LIB_EXT)
SRC = disk_space.c

all: $(TARGET)

$(TARGET): $(SRC) | $(PRIV_DIR)
	$(CC) -fPIC -I"$(ERL_INCLUDE_DIR)" -shared $(SRC) -o $(TARGET)

$(PRIV_DIR):
	mkdir -p $(PRIV_DIR)

clean:
	rm -f $(TARGET)
