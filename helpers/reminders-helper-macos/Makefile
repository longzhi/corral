.PHONY: all clean

all: reminders-helper

reminders-helper: Sources/main.swift
	swiftc -o reminders-helper Sources/main.swift \
		-framework EventKit \
		-framework Foundation \
		-O

clean:
	rm -f reminders-helper

test: reminders-helper
	@echo '{"action":"list"}' | ./reminders-helper
