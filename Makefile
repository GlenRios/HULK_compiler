# Makefile — compilador HULK
#
# Dependencias en Ubuntu 24.04 LTS:
#   sudo apt-get install -y build-essential llvm-17 llvm-17-dev curl
#   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
#   source "$HOME/.cargo/env"
#
# Uso:
#   make build   — produce ./hulk y ./hulk_runtime.a
#   make clean   — elimina artefactos

.PHONY: build clean

RUNTIME_SRC = runtime/hulk_runtime.c
RUNTIME_OBJ = /tmp/hulk_runtime.o
RUNTIME_LIB = hulk_runtime.a

# Detectar LLVM 17.  En Ubuntu, llvm-17-dev instala /usr/lib/llvm-17.
# Se puede sobreescribir: make build LLVM_PREFIX=/ruta/a/llvm17
LLVM_PREFIX ?= /usr/lib/llvm-17

build:
	@# Verificar / instalar LLVM 17 en Ubuntu si no está presente
	@if [ ! -f "$(LLVM_PREFIX)/bin/llvm-config" ]; then \
		echo "[hulk] LLVM 17 no encontrado en $(LLVM_PREFIX), instalando..."; \
		apt-get install -y llvm-17 llvm-17-dev; \
	fi
	@# Compilar el compilador Rust
	LLVM_SYS_170_PREFIX=$(LLVM_PREFIX) cargo build --release
	cp target/release/hulk_compiler ./hulk
	@# Compilar la biblioteca de runtime en C
	gcc -O2 -c $(RUNTIME_SRC) -o $(RUNTIME_OBJ) -lm
	ar rcs $(RUNTIME_LIB) $(RUNTIME_OBJ)
	@echo "[hulk] Build completado: ./hulk y ./$(RUNTIME_LIB) listos."

clean:
	cargo clean
	rm -f ./hulk $(RUNTIME_LIB) $(RUNTIME_OBJ) ./output
