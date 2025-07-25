.PHONY: help
help: makefile
	@tail -n +4 makefile | grep ".PHONY"


.PHONY: format
format:
	cargo clippy --fix --allow-dirty
	cargo fmt
	find . -type f -name '*.rs' \
		-exec sed -i -E 's/^([[:space:]]*)\} else/\1}\n\1else/g' {} +


.PHONY: test
test: format
	cargo test -- --show-output


.PHONY: install
install:
	cargo install --path .


.PHONY: release
release:
	@echo '1. `cai changelog <first-new-commit-hash>`'
	@echo '2. `git add ./changelog.md && git commit -m "Update changelog"`'
	@echo '3. `cargo release major / minor / patch`'
	@echo '4. Create a new GitHub release at https://github.com/ad-si/tu/releases/new'
	@echo \
		"5. Announce release on \n" \
		"   - https://x.com \n" \
		"   - https://bsky.app \n" \
		"   - https://this-week-in-rust.org \n" \
		"   - https://news.ycombinator.com \n" \
		"   - https://lobste.rs \n" \
		"   - https://reddit.com/r/rust \n"
