use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chimera_parser::{Parser, ParseOptions};
use chimera_source::SourceFileId;
use std::fs::read_to_string;

/// Create a complex Elixir module for parsing benchmark
fn create_complex_module_source() -> String {
    r#"
defmodule Math do
  @moduledoc """
  Provides mathematical functions for working with numbers.
  """

  @spec add(integer, integer) :: integer
  def add(a, b) do
    a + b
  end

  @spec multiply(number, number) :: number
  def multiply(a, b) when is_number(a) and is_number(b) do
    a * b
  end

  def factorial(0), do: 1
  def factorial(n) when n > 0 do
    n * factorial(n - 1)
  end

  def fibonacci(0), do: 0
  def fibonacci(1), do: 1
  def fibonacci(n) when n > 1 do
    fibonacci(n - 1) + fibonacci(n - 2)
  end

  defprocess loop(state) do
    receive do
      {:compute, {op, a, b}} ->
        result = case op do
          {:add, 2} -> add(a, b)
          {:mul, 2} -> multiply(a, b)
          _ -> {:error, :unsupported_operation}
        end
        loop(Map.put(state, :last_result, result))
      {:get_result, sender} ->
        send(sender, Map.get(state, :last_result))
        loop(state)
    end
  end
end
"#.to_string()
}

fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");
    group.sample_size(100);
    
    group.bench_function("parse_complex_module", |b| {
        let source = create_complex_module_source();
        b.iter(|| {
            let mut parser = Parser::new(SourceFileId::new(0), &source, ParseOptions::default());
            let result = parser.parse().unwrap();
            black_box(result);
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_parser);
criterion_main!(benches);