use criterion::{black_box, criterion_group, criterion_main, Criterion};
use chimera_lexer::{Lexer, LexerOptions};
use chimera_source::SourceFileId;

/// Create a complex Elixir source for lexing benchmark
fn create_complex_source() -> String {
    r#"
defmodule Benchmark do
  @moduledoc """
  This is a complex module for benchmarking the lexer.
  It contains various tokens, operators, and syntactic elements.
  """

  @spec fibonacci(integer) :: integer
  def fibonacci(n) when n < 2 do
    n
  end
  
  def fibonacci(n) do
    fibonacci(n-1) + fibonacci(n-2)
  end

  def process_data(data) do
    for {:ok, item} <- data, do: 
      case item do
        {:error, reason} -> {:error, reason}
        value when is_number(value) -> value * 2
        _ -> :ignore
      end
  end

  def __using__(_options) do
    quote do
      import __MODULE__
      
      @before_compile __MODULE__
      
      def __using__(_) do
        quote do
          def test() do
            IO.inspect("Working")
          end
        end
      end
    end
  end
end
"#.to_string()
}

fn bench_lexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer");
    group.sample_size(100);
    
    group.bench_function("lex_complex_source", |b| {
        let source = create_complex_source();
        b.iter(|| {
            let mut lexer = Lexer::new(SourceFileId::new(0), &source, LexerOptions::default());
            let tokens: Vec<_> = lexer.collect();
            black_box(tokens.len());
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_lexer);
criterion_main!(benches);