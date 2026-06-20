/*
 * hulk_runtime.c — Runtime de HULK para binarios AOT (compilados a objeto).
 *
 * Este archivo implementa en C las mismas funciones que runtime.rs implementa
 * en Rust para el JIT. Son dos implementaciones del mismo contrato:
 *
 *   - runtime.rs  → usada en modo JIT (desarrollo): las funciones viven en el
 *                   proceso del compilador y el JIT las resuelve por dlsym.
 *   - hulk_runtime.c → usada en modo AOT (producción): se compila a
 *                   hulk_runtime.a y se linkea con el .o del programa HULK.
 *
 * El compilador emite "declare" de cada función (External linkage), así que
 * el linker las encuentra en este .a.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdint.h>

/* ── Strings ─────────────────────────────────────────────────────────────── */

void hulk_print(const char *s) {
    if (!s) { puts("null"); return; }
    printf("%s\n", s);
}

/* Convierte un double a string. Si es entero exacto, sin decimales. */
char *hulk_str_from_number(double n) {
    char *buf = (char *)malloc(64);
    if (!buf) abort();
    long long as_int = (long long)n;
    if ((double)as_int == n && fabs(n) < 1e15)
        snprintf(buf, 64, "%lld", as_int);
    else
        snprintf(buf, 64, "%g", n);
    return buf;
}

char *hulk_str_concat(const char *a, const char *b) {
    size_t la = a ? strlen(a) : 0;
    size_t lb = b ? strlen(b) : 0;
    char *r = (char *)malloc(la + lb + 1);
    if (!r) abort();
    if (a) memcpy(r, a, la);
    if (b) memcpy(r + la, b, lb);
    r[la + lb] = '\0';
    return r;
}

char *hulk_str_concat_space(const char *a, const char *b) {
    size_t la = a ? strlen(a) : 0;
    size_t lb = b ? strlen(b) : 0;
    char *r = (char *)malloc(la + 1 + lb + 1);
    if (!r) abort();
    if (a) memcpy(r, a, la);
    r[la] = ' ';
    if (b) memcpy(r + la + 1, b, lb);
    r[la + 1 + lb] = '\0';
    return r;
}

double hulk_str_size(const char *s) {
    if (!s) return 0.0;
    return (double)strlen(s);
}

/* Igualdad por valor (no por puntero). */
int hulk_str_eq(const char *a, const char *b) {
    if (!a && !b) return 1;
    if (!a || !b) return 0;
    return strcmp(a, b) == 0;
}

/* ── Números ─────────────────────────────────────────────────────────────── */

double hulk_rand(void) {
    return (double)rand() / ((double)RAND_MAX + 1.0);
}

/* ── Vectores ────────────────────────────────────────────────────────────── */
/*
 * Layout en memoria: [int64 count][elemento_0: 8 bytes][elemento_1: 8 bytes]...
 * Cada elemento ocupa 8 bytes independientemente del tipo (f64, ptr, bool
 * extendido a i64). hulk_vec_get devuelve un puntero al slot → el compilador
 * emite el store/load del tipo correcto.
 */

void *hulk_vec_alloc(int count, int elem_size) {
    if (count < 0) count = 0;
    size_t n = (size_t)count;
    void *p = calloc(1, 8 + n * 8);
    if (!p) abort();
    *(int64_t *)p = (int64_t)n;
    return p;
}

void *hulk_vec_get(void *vec, int index, int elem_size) {
    int64_t size = *(int64_t *)vec;
    if (index < 0 || (int64_t)index >= size) {
        fprintf(stderr, "HULK runtime error: index %d out of range [0, %lld)\n",
                index, (long long)size);
        abort();
    }
    return (char *)vec + 8 + (size_t)index * 8;
}

double hulk_vec_size(void *vec) {
    return (double)*(int64_t *)vec;
}

/* ── Range ───────────────────────────────────────────────────────────────── */
/*
 * Layout: [f64 start][f64 end][f64 current]
 * current empieza en start-1 para que el primer hulk_range_next() lo lleve a start.
 */

void *hulk_range_alloc(double start, double end) {
    double *p = (double *)malloc(3 * sizeof(double));
    if (!p) abort();
    p[0] = start; p[1] = end; p[2] = start - 1.0;
    return p;
}

int hulk_range_next(void *range) {
    double *p = (double *)range;
    p[2] += 1.0;
    return p[2] < p[1];
}

double hulk_range_current(void *range) {
    return ((double *)range)[2];
}

/* ── Manejo de errores ───────────────────────────────────────────────────── */

void hulk_type_error(const char *msg) {
    if (msg) fprintf(stderr, "%s\n", msg);
    abort();
}
