# dg 빌드·설치
PREFIX ?= $(HOME)/.local
BINDIR  = $(PREFIX)/bin
CARGO  ?= cargo
BIN     = target/release/dg

MAC_HOST ?= mac

.PHONY: all build test lint install uninstall clean deploy-mac examples

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

# examples/architecture.md를 실제로 렌더링해 examples/architecture.txt로 저장한다(README가 링크하는
# "변환된 파일"). architecture.md를 고치면 이 타깃으로 다시 만들어야 둘이 어긋나지 않는다.
examples: build
	$(BIN) -P -s none -w 100 examples/architecture.md > examples/architecture.txt

# 소스를 Mac으로 보내 그쪽 cargo로 빌드·설치 (~/.cargo/bin/dg). 교차 컴파일 도구가 필요 없다.
deploy-mac:
	rsync -az --delete --exclude target --exclude .git ./ $(MAC_HOST):tools/dg/
	ssh $(MAC_HOST) 'cd ~/tools/dg && cargo install --path . --locked && ~/.cargo/bin/dg --version'
