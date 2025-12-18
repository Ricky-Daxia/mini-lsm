// Copyright (c) 2022-2025 Alex Chi Z
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use std::sync::Arc;

use bytes::{Buf, Bytes};

use crate::key::{KeySlice, KeyVec};

use super::Block;

const KEY_VAL_LEN: usize = 4; // 2 bytes for key length and 2 bytes for value length

/// Iterates on a block.
pub struct BlockIterator {
    /// The internal `Block`, wrapped by an `Arc`
    block: Arc<Block>,
    /// The current key, empty represents the iterator is invalid
    key: KeyVec,
    /// the current value range in the block.data, corresponds to the current key
    value_range: (usize, usize),
    /// Current index of the key-value pair, should be in range of [0, num_of_elements)
    idx: usize,
    /// The first key in the block
    first_key: KeyVec,
}

impl Block {
    fn get_first_key(&self) -> KeyVec {
        let mut buf = &self.data[..];
        let key_len = buf.get_u16();
        let key = &buf[..key_len as usize];
        KeyVec::from_vec(key.to_vec())
    }
}

impl BlockIterator {
    fn new(block: Arc<Block>) -> Self {
        Self {
            first_key: block.get_first_key(),
            block,
            key: KeyVec::new(),
            value_range: (0, 0),
            idx: 0,
        }
    }

    /// Creates a block iterator and seek to the first entry.
    pub fn create_and_seek_to_first(block: Arc<Block>) -> Self {
        if block.data.is_empty() || block.offsets.is_empty() {
            return Self::new(block);
        }
        let mut iter = Self::new(block);
        iter.seek_to_first();
        iter
    }

    /// Creates a block iterator and seek to the first key that >= `key`.
    pub fn create_and_seek_to_key(block: Arc<Block>, key: KeySlice) -> Self {
        if block.data.is_empty() || block.offsets.is_empty() {
            return Self::new(block);
        }
        let mut iter = Self::new(block);
        iter.seek_to_key(key);
        iter
    }

    /// Returns the key of the current entry.
    pub fn key(&self) -> KeySlice<'_> {
        self.key.as_key_slice()
    }

    /// Returns the value of the current entry.
    pub fn value(&self) -> &[u8] {
        &self.block.data[self.value_range.0..self.value_range.1]
    }

    /// Returns true if the iterator is valid.
    /// Note: You may want to make use of `key`
    pub fn is_valid(&self) -> bool {
        !self.key.is_empty()
    }

    /// Seeks to the first key in the block.
    pub fn seek_to_first(&mut self) {
        if self.block.data.is_empty() || self.block.offsets.is_empty() {
            self.key.clear();
            self.value_range = (0, 0);
            return;
        }
        let first_key_offset = self.block.offsets[0] as usize;
        let mut data_buf = &self.block.data[first_key_offset..];
        let key_len = data_buf.get_u16() as usize;
        let key = Bytes::copy_from_slice(&data_buf[..key_len]);
        self.key = KeyVec::from_vec(key.to_vec());
        data_buf.advance(key_len);
        let value_len = data_buf.get_u16() as usize;
        let value_start = first_key_offset + KEY_VAL_LEN + key_len;
        self.value_range = (value_start, value_start + value_len);
        self.idx = 0;
    }

    /// Move to the next key in the block.
    pub fn next(&mut self) {
        self.idx += 1;
        if self.idx >= self.block.offsets.len() {
            self.key.clear();
            self.value_range = (0, 0);
            return;
        }
        let key_offset = self.block.offsets[self.idx] as usize;
        let mut data_buf = &self.block.data[key_offset..];
        let key_len = data_buf.get_u16() as usize;
        let key = Bytes::copy_from_slice(&data_buf[..key_len]);
        self.key = KeyVec::from_vec(key.to_vec());
        data_buf.advance(key_len);
        let value_len = data_buf.get_u16() as usize;
        let value_start = key_offset + KEY_VAL_LEN + key_len;
        self.value_range = (value_start, value_start + value_len);
    }

    /// Seek to the first key that >= `key`.
    /// Note: You should assume the key-value pairs in the block are sorted when being added by
    /// callers.
    pub fn seek_to_key(&mut self, key: KeySlice) {
        if self.block.data.is_empty() || self.block.offsets.is_empty() {
            self.key.clear();
            self.value_range = (0, 0);
            return;
        }
        let (data, offsets) = (&self.block.data, &self.block.offsets);
        let idx = offsets.partition_point(|offset| {
            let mut buf = &data[*offset as usize..];
            let key_len = buf.get_u16() as usize;
            let key_bytes = &buf[..key_len];
            KeySlice::from_slice(key_bytes) < key
        });
        if idx >= offsets.len() {
            self.key.clear();
            self.value_range = (0, 0);
            return;
        }
        let key_offset = offsets[idx] as usize;
        let mut data_buf = &data[key_offset..];
        let key_len = data_buf.get_u16() as usize;
        let key = Bytes::copy_from_slice(&data_buf[..key_len]);
        self.key = KeyVec::from_vec(key.to_vec());
        data_buf.advance(key_len);
        let value_len = data_buf.get_u16() as usize;
        let value_start = key_offset + KEY_VAL_LEN + key_len;
        self.value_range = (value_start, value_start + value_len);
        self.idx = idx;
    }
}
