#!/usr/bin/env ruby
# frozen_string_literal: true

# Checks that the Acme Shop services answer their health endpoints.
# Run it inside the compose network:
#   docker compose run --rm -v "$PWD/tools:/tools" ruby:3.3 ruby /tools/healthcheck.rb
# Exits 0 when every check passes, 1 otherwise.

require 'net/http'
require 'json'
require 'optparse'
require 'uri'

CHECKS = {
  'orders' => 'http://orders:8080/healthz',
  'storefront' => 'http://storefront:3000/healthz'
}.freeze

def check(name, url, timeout)
  uri = URI(url)
  res = Net::HTTP.start(uri.host, uri.port, open_timeout: timeout, read_timeout: timeout) do |http|
    http.get(uri.request_uri)
  end
  status = begin
    JSON.parse(res.body).fetch('status', 'unknown')
  rescue JSON::ParserError
    'unparseable'
  end
  { name: name, url: url, ok: res.is_a?(Net::HTTPSuccess), code: res.code.to_i, status: status }
rescue StandardError => e
  { name: name, url: url, ok: false, error: "#{e.class}: #{e.message}" }
end

timeout = 2
OptionParser.new do |opts|
  opts.banner = 'Usage: healthcheck.rb [--timeout SECONDS]'
  opts.on('--timeout SECONDS', Integer, 'Per-request timeout (default 2)') { |t| timeout = t }
end.parse!

results = CHECKS.map { |name, url| check(name, url, timeout) }
results.each do |r|
  if r[:ok]
    puts "ok   #{r[:name]} #{r[:code]} #{r[:status]}"
  else
    puts "FAIL #{r[:name]} #{r[:error] || r[:code]}"
  end
end
exit(results.all? { |r| r[:ok] } ? 0 : 1)
