// Generated C++17 caller driver. The generated move-only wrapper (over the
// generated C11 client) performs every boundary operation; this file only
// supplies host values and prints the shared receipt.
#include "include/semaprax_public_generic_v1.hpp"
#include <cstdint>
#include <cstdlib>
#include <string>
using namespace semaprax::public_generic::v1;
static void mx_run(const mx_case *c) {
    auto opened = Provider::open();
    if (!opened) std::exit(3);
    Provider provider = std::move(opened).value();
    const unsigned cycles = c->kind == MX_REPEATED ? c->cycles : 1;
    for (unsigned cycle = 0; cycle < cycles; ++cycle) {
        Input input;
        input.@INPUT0@.assign(c->left, c->left + c->left_len);
        input.@INPUT1@.assign(c->right, c->right + c->right_len);
        mx_arm(c->kind);
        auto result = provider.transform(std::move(input));
        std::string note = "wrapper";
        const bool last = !(c->kind == MX_REPEATED && cycle + 1 < cycles);
        auto finish = [&](long primary, long secondary, bool present, const std::uint8_t *l,
                          std::size_t ln, const std::uint8_t *r, std::size_t rn) {
            if (last) {
                const bool closed = static_cast<bool>(provider.close_checked());
                note += std::string(",close=") + (closed && !provider.valid() ? "0" : "1") +
                    ",armed=" + std::to_string(mx_armed());
            }
            mx_emit(c, cycle, primary, secondary, present, l, ln, r, rn, note.c_str());
        };
        if (result) {
            Output output = std::move(result).value();
            const BytesView first = output.@OUTPUT0@(), second = output.@OUTPUT1@();
            finish(0, -1, true, first.data, first.size, second.data, second.size);
        } else {
            note += ",kind=" + std::to_string(static_cast<unsigned>(result.error().kind()));
            finish(result.error().native_status(), result.error().release_status(), false,
                nullptr, 0, nullptr, 0);
        }
    }
}
int main() {
    for (std::size_t i = 0; i < MX_CASE_COUNT; ++i) {
        mx_reset();
        mx_run(&mx_cases[i]);
    }
    return 0;
}
