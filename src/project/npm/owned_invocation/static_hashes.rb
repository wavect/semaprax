# DATA-ONLY review recipe. Reads literal templates and hashes their expansion;
# never evaluates JavaScript/Rust, compiles, instantiates Wasm, or writes files.
# Run from repository root: ruby src/project/npm/owned_invocation/static_hashes.rb
require 'digest'

BASE = '3983d85'.freeze
FILE = 'src/project/npm/owned_data.rs'.freeze
DIR = 'src/project/npm/owned_invocation'.freeze

def historical(path)
  bytes = IO.popen(['git', 'show', "#{BASE}:#{path}"], &:read)
  raise 'cannot read frozen Git source' unless $?.success?
  bytes
end

def single(source, expression)
  matches = source.scan(expression)
  raise 'literal capture must be unique' unless matches.length == 1
  matches.first.first
end

# This is one pass over Rust format syntax. Inserted JS braces are not reparsed.
def expand(template, substitutions)
  template.gsub(/\{\{|\}\}|\{([a-z_][a-z_0-9]*)\}/) do |token|
    token == '{{' ? '{' : token == '}}' ? '}' : substitutions.fetch(Regexp.last_match(1))
  end
end

def body(source, name)
  # Every old renderer ends at a line containing exactly `}`. Nested Rust
  # blocks are indented, and JS braces are inside the captured raw literal.
  single(source, /(?:pub\(super\) )?fn #{Regexp.escape(name)}\([^\n]*\n(.*?)^\}/m)
end

def raw_format(function)
  single(function, /format!\(\s*r#"(.*?)"#/m)
end

baseline = historical(FILE)
current = File.binread(FILE)
legacy = single(baseline, /const LEGACY_INPUT_PRELUDE: &str = r#"(.*?)"#;/m)
raise 'historical input changed' unless legacy == single(current, /const LEGACY_INPUT_PRELUDE: &str = r#"(.*?)"#;/m)
input = historical('src/project/npm/owned_data_input_v8.js')
raise 'bounded input changed' unless input == File.binread('src/project/npm/owned_data_input_v8.js')

pins = {
  'render_runtime_prelude_with_admission' => ['4d0057aed9591b91ea9ef11f84657ca6be1db45dd6d3d3afdd9b6c2bfe19e61f', '9f031e17da0d1c125d0fd8ebf54171e69d44a44b24931d8e2a945048577a7e1b'],
  'render_runtime_facade' => ['51414bc65b07f7dc7f83bb5ecbc7b8958f61cbd55e5929b3babcf37d537850cc', '758225ba0a123e21c391ab1da56fbb23d4c1870d7b5387fbba8ffa6bc390d716'],
  'render_variant_runtime_facade' => ['b10112d3d1cb8640d02d7a536626ff12ee975d53e7a181642a9e0ae3e791b20f', '1e5eeb39071283bcdcdd0c90ccef2cfc075c89f81b310a9b783cad089850a4fb'],
  'render_mixed_runtime_facade' => ['9b55a36b641c28dfcd77b1658c1fd610716d30a30f304b1cc9b6660e41aaa457', 'a64b98625048e72fea3891b6d199e89a6cc628575b285585e3efd257dd8a9f20']
}.freeze

pins.each do |name, expected|
  old_body = body(baseline, name)
  template = raw_format(old_body)
  raise "historical false template changed: #{name}" unless template == raw_format(body(current, name))
  [false, true].each_with_index do |bounded, index|
    substitutions = {'digest'=>'"digest"', 'capacity'=>'16', 'facts'=>'', 'memory_bytes'=>'131072', 'decoder_declaration'=>'', 'utf8_case'=>''}
    if name == 'render_runtime_prelude_with_admission'
      substitutions['input_prelude'] = bounded ? input : legacy
    else
      prefix = old_body.split('    format!(', 2).first
      substitutions['input_admission'] = bounded ? 'const {snapshots,used}=snapshotArguments(values,fact.params);' : single(prefix, /r#"(.*?)"#/m)
    end
    hash = Digest::SHA256.hexdigest(expand(template, substitutions))
    raise "calibration mismatch: #{name} #{bounded}: #{hash}" unless hash == expected[index]
  end
end
puts 'calibrated all eight historical literal-template hashes; false templates unchanged'

arena = File.binread("#{DIR}/arena.js")
raise 'capacity marker must be unique' unless arena.scan('__SPX_CAPACITY__').length == 1
prelude = "const EXPECTED_WASM_SHA256 = \"digest\";\n" + input + arena.sub('__SPX_CAPACITY__', '16')
%w[core call result].each { |name| prelude += File.binread("#{DIR}/#{name}.js") }
facade = File.binread("#{DIR}/facade.js")
%w[__SPX_FACTS__ __SPX_MEMORY_BYTES__].each do |marker|
  raise 'facade marker must be unique' unless facade.scan(marker).length == 1
end
facade = facade.sub('__SPX_MEMORY_BYTES__', '131072').sub('__SPX_FACTS__', '')
puts "bounded prelude: #{Digest::SHA256.hexdigest(prelude)}"
puts "bounded direct/variant/mixed empty facade: #{Digest::SHA256.hexdigest(facade)}"
