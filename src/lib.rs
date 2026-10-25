//! AI Chat Client 核心库：数据模型、持久化、大模型 API 与编排服务。
//! UI（app / ui 模块）保留在二进制 crate 中，以便库可被独立测试。

pub mod api;
pub mod config;
pub mod db;
pub mod model;
pub mod service;
