# dg 빌드·설치
PREFIX ?= $(HOME)/.local
BINDIR  = $(PREFIX)/bin
CARGO  ?= cargo
BIN     = target/release/dg

MAC_HOST ?= mac

.PHONY: all build test lint install uninstall clean deploy-mac

all: build

build:
	$(CARGO) build --release

test:
	$(CARGO) test

lint:
	$(CARGO) clippy -- -D warnings

install: build
	install -d $(BINDIR)
	install -m 755 $(BIN) $(BINDIR)/dg

uninstall:
	rm -f $(BINDIR)/dg

clean:
	$(CARGO) clean

# 소스를 Mac으로 보내 그쪽 cargo로 빌드·설치 (~/.cargo/bin/dg). 교차 컴파일 도구가 필요 없다.
deploy-mac:
	rsync -az --delete --exclude target --exclude .git ./ $(MAC_HOST):tools/dg/
	ssh $(MAC_HOST) 'cd ~/tools/dg && cargo install --path . --locked && ~/.cargo/bin/dg --version'
