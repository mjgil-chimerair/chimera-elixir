const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Test module
    const test_module = b.createModule(.{
        .root_source_file = b.path("src/utf8.zig"),
        .target = target,
        .optimize = optimize,
    });

    test_module.addImport("bitstring", b.createModule(.{
        .root_source_file = b.path("src/bitstring.zig"),
        .target = target,
        .optimize = optimize,
    }));

    test_module.addImport("source_buffer", b.createModule(.{
        .root_source_file = b.path("src/source_buffer.zig"),
        .target = target,
        .optimize = optimize,
    }));

    test_module.addImport("etf", b.createModule(.{
        .root_source_file = b.path("src/etf.zig"),
        .target = target,
        .optimize = optimize,
    }));

    test_module.addImport("scanner", b.createModule(.{
        .root_source_file = b.path("src/scanner.zig"),
        .target = target,
        .optimize = optimize,
    }));

    const tests = b.addTest(.{
        .root_module = test_module,
    });

    const run_tests = b.addRunArtifact(tests);

    const test_step = b.step("test", "Run library tests");
    test_step.dependOn(&run_tests.step);

    b.default_step.dependOn(test_step);
}