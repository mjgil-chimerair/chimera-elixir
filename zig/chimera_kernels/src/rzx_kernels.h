/* Rust/Zig Elixir Compiler - Kernel ABI Header
 *
 * This header defines the C ABI for Zig kernels callable from Rust.
 * All kernels must follow these safety rules:
 * - No Rust pointers retained
 * - No compiler state modified
 * - Bounded buffer operations
 */

#ifndef RZX_KERNELS_H
#define RZX_KERNELS_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Error codes */
typedef enum {
    RZX_SUCCESS = 0,
    RZX_INVALID_UTF8 = 1,
    RZX_INVALID_OFFSET = 2,
    RZX_BUFFER_TOO_SMALL = 3,
    RZX_INVALID_ESCAPE = 4,
    RZX_UNTERMINATED_STRING = 5,
    RZX_INVALID_CHARACTER = 6,
} rzx_error_t;

/* UTF-8 kernel results */
typedef struct {
    uint32_t start_offset;
    uint32_t end_offset;
    uint32_t success;
} rzx_scan_result_t;

/* Line/column result */
typedef struct {
    uint32_t line;
    uint32_t col;
} rzx_line_col_t;

/* Source span */
typedef struct {
    uint32_t start;
    uint32_t end;
} rzx_span_t;

/* Bitstring segment options */
typedef struct {
    uint32_t size;
    uint32_t unit;
    uint32_t type_flag;
    uint32_t signed_flag;
    uint32_t big_endian;
    uint32_t literal;
} rzx_bitstring_opts_t;

/* Bitstring segment result */
typedef struct {
    uint32_t offset;
    uint32_t bits;
    uint32_t success;
    uint32_t error_code;
} rzx_bitstring_result_t;

/* ETF result */
typedef struct {
    uint32_t bytes_consumed;
    uint32_t success;
    uint32_t error_code;
} rzx_etf_result_t;

/* UTF-8 kernels */
uintptr_t rzx_utf8_validate(const uint8_t* data, size_t len);
uint32_t rzx_utf8_is_valid(const uint8_t* data, size_t len);
rzx_scan_result_t rzx_scan_identifier(const uint8_t* data, size_t len, size_t start, size_t end);
rzx_scan_result_t rzx_scan_alias(const uint8_t* data, size_t len, size_t start);
rzx_line_col_t rzx_offset_to_line_col(const uint8_t* data, size_t len, size_t offset);

/* Source buffer kernels */
rzx_span_t rzx_span_create(uint32_t start, uint32_t end);
uint32_t rzx_span_is_valid(const uint8_t* data, size_t len, uint32_t start, uint32_t end);
rzx_span_t rzx_span_merge(rzx_span_t span1, rzx_span_t span2);
uint32_t rzx_span_text(const uint8_t* data, uint32_t start, uint32_t end, uint8_t* out_buf, size_t out_len);
uint32_t rzx_find_next_newline(const uint8_t* data, size_t len, size_t offset);
uint32_t rzx_find_prev_newline(const uint8_t* data, size_t offset);
uint32_t rzx_count_newlines(const uint8_t* data, size_t offset);
uint32_t rzx_line_offset(const uint8_t* data, size_t len, uint32_t line);
uint32_t rzx_span_is_empty(rzx_span_t span);
uint32_t rzx_span_length(rzx_span_t span);

/* Bitstring kernels */
rzx_bitstring_result_t rzx_bitstring_parse_segment(const uint8_t* data, size_t len, size_t offset, rzx_bitstring_opts_t opts);
uint64_t rzx_bitstring_calculate_size(const uint32_t* segment_sizes, size_t segment_count);
uint32_t rzx_bitstring_validate_opts(const uint8_t* opts_str);

/* ETF kernels */
rzx_etf_result_t rzx_etf_decode_small_int(const uint8_t* buf, size_t len);
rzx_etf_result_t rzx_etf_decode_atom(const uint8_t* buf, size_t len);
rzx_etf_result_t rzx_etf_decode_nil(const uint8_t* buf, size_t len);
rzx_etf_result_t rzx_etf_decode_cons(const uint8_t* buf, size_t len);
rzx_etf_result_t rzx_etf_decode_string(const uint8_t* buf, size_t len);
rzx_etf_result_t rzx_etf_decode_binary(const uint8_t* buf, size_t len);
uint32_t rzx_etf_encode_nil(uint8_t* buf, size_t len);
uint32_t rzx_etf_encode_small_int(uint8_t* buf, size_t len, uint32_t value);
uint8_t rzx_etf_version(void);
uint32_t rzx_etf_estimate_size(uint32_t term_type);

#ifdef __cplusplus
}
#endif

#endif /* RZX_KERNELS_H */