//! LLM integration: OpenAI-compatible client, query expansion, summarization.

#[cfg_attr(docsrs, doc(cfg(feature = "llm")))]
pub mod client;
#[cfg_attr(docsrs, doc(cfg(feature = "llm")))]
pub mod expander;
#[cfg_attr(docsrs, doc(cfg(feature = "llm")))]
pub mod summarizer;
