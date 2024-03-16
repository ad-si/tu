.PHONY: help
help: makefile
	@tail -n +4 makefile | grep ".PHONY"


.PHONY: test
test:
	cargo test -- --show-output


.PHONY: install
install:
	cargo install --path .
