#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "open3"
require "pathname"

repository = Pathname.new(__dir__).join("../..").cleanpath
compiler = Pathname.new(ENV.fetch("JADPO_BIN", repository.join("jadpo/target/debug/jadpo").to_s))
abort("compiler not found: #{compiler}") unless compiler.file?

expectations = Dir[repository.join("tests/compile/**/*.expect.json").to_s].sort
sources = Dir[repository.join("tests/compile/**/*.jadpo").to_s].sort
expected_sources = expectations.map { |path| path.sub(/\.expect\.json\z/, ".jadpo") }
abort("fixture source/expectation sets differ") unless sources == expected_sources

failures = []
expectations.each do |expectation_path|
  source_path = expectation_path.sub(/\.expect\.json\z/, ".jadpo")
  relative_source = Pathname.new(source_path).relative_path_from(repository).to_s
  expectation = JSON.parse(File.read(expectation_path))
  stdout, stderr, status = Open3.capture3(
    compiler.to_s,
    "check",
    relative_source,
    "--diagnostic-format=json",
    chdir: repository.to_s
  )

  begin
    report = JSON.parse(stdout)
  rescue JSON::ParserError => error
    failures << "#{relative_source}: invalid compiler JSON: #{error.message}; stderr=#{stderr.inspect}"
    next
  end

  expected_pass = expectation.fetch("result") == "pass"
  actual_pass = status.success? && report["status"] == "passed"
  if expected_pass != actual_pass
    failures << "#{relative_source}: expected #{expectation.fetch("result")}, got #{report["status"]} (exit #{status.exitstatus})"
    next
  end

  expected_diagnostics = expectation.fetch("diagnostics", [])
  actual_diagnostics = report.fetch("diagnostics", []).select { |diagnostic| diagnostic["severity"] == "error" }
  actual_codes = actual_diagnostics.map { |diagnostic| diagnostic.fetch("legacyAliases", []).first }
  expected_codes = expected_diagnostics.map { |diagnostic| diagnostic.fetch("code") }
  if actual_codes != expected_codes
    failures << "#{relative_source}: expected diagnostic codes #{expected_codes.inspect}, got #{actual_codes.inspect}"
    next
  end

  source = File.read(source_path)
  expected_diagnostics.zip(actual_diagnostics).each do |expected, actual|
    next unless expected.key?("primary_match")

    location = actual.fetch("location")
    range = location.fetch("range")
    actual_match = source.byteslice(range.fetch("start")...range.fetch("end"))
    next if actual_match == expected.fetch("primary_match")

    failures << "#{relative_source}: #{expected.fetch("code")} expected primary #{expected.fetch("primary_match").inspect}, got #{actual_match.inspect}"
  end
end

abort(failures.join("\n")) unless failures.empty?

puts "verified #{expectations.length} compile fixture pairs"
