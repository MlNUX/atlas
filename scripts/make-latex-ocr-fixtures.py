#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Minux
# SPDX-License-Identifier: GPL-3.0-or-later
"""Tiny stand-ins for the pix2tex ONNX models, for the tests of
`atlas_core::latex_ocr` (the real models are about 100 MB).

Usage: make-latex-ocr-fixtures.py DIR   (needs the Python package `onnx`)

They have the inputs and outputs of the RapidLaTeXOCR export:

- image_resizer.onnx: picture [1, 1, H, W] → logits [1, 22]; the best class
  is W/32 − 1, i.e. "the width is right" (the resize loop stops at once);
- encoder.onnx: picture → context [1, 4, 8];
- decoder.onnx: tokens x [1, t] (int64), mask [1, t] (bool), context →
  logits [1, t, V]; the token after position i is SEQ[i + 1], so greedy
  decoding yields SEQ (ending with [EOS]);
- tokenizer.json: the vocabulary.
"""

import json
import sys
from pathlib import Path

import numpy as np
from onnx import TensorProto, helper, numpy_helper, save

VOCAB = ["[PAD]", "[BOS]", "[EOS]", "\\frac", "{", "a", "}", "b", "Ġ+", "x"]
# [BOS] \frac { a } { b } [EOS]
SEQ = [1, 3, 4, 5, 6, 4, 7, 6, 2]
V = len(VOCAB)


def const(name, array):
    return helper.make_node("Constant", [], [name], value=numpy_helper.from_array(np.asarray(array), name + "_v"))


def model(nodes, inputs, outputs, name):
    graph = helper.make_graph(nodes, name, inputs, outputs)
    m = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 17)], producer_name="atlas-test")
    m.ir_version = 8
    return m


def picture():
    return helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 1, "H", "W"])


def resizer():
    nodes = [
        helper.make_node("Shape", ["input"], ["shape"]),
        const("three", np.array(3, dtype=np.int64)),
        helper.make_node("Gather", ["shape", "three"], ["w"], axis=0),
        const("c32", np.array(32, dtype=np.int64)),
        helper.make_node("Div", ["w", "c32"], ["w32"]),
        const("one", np.array(1, dtype=np.int64)),
        helper.make_node("Sub", ["w32", "one"], ["best"]),
        const("classes", np.arange(22, dtype=np.int64)),
        helper.make_node("Equal", ["classes", "best"], ["hit"]),
        helper.make_node("Cast", ["hit"], ["hitf"], to=TensorProto.FLOAT),
        const("axes0", np.array([0], dtype=np.int64)),
        helper.make_node("Unsqueeze", ["hitf", "axes0"], ["logits"]),
    ]
    out = helper.make_tensor_value_info("logits", TensorProto.FLOAT, [1, 22])
    return model(nodes, [picture()], [out], "resizer")


def encoder():
    nodes = [
        helper.make_node("ReduceMean", ["input"], ["mean"], keepdims=0),
        const("base", np.ones((1, 4, 8), dtype=np.float32)),
        helper.make_node("Add", ["base", "mean"], ["context"]),
    ]
    out = helper.make_tensor_value_info("context", TensorProto.FLOAT, [1, 4, 8])
    return model(nodes, [picture()], [out], "encoder")


def decoder():
    x = helper.make_tensor_value_info("x", TensorProto.INT64, [1, "t"])
    mask = helper.make_tensor_value_info("mask", TensorProto.BOOL, [1, "t"])
    context = helper.make_tensor_value_info("context", TensorProto.FLOAT, [1, 4, 8])
    nodes = [
        helper.make_node("Shape", ["x"], ["shape"]),
        const("one_i", np.array(1, dtype=np.int64)),
        helper.make_node("Gather", ["shape", "one_i"], ["t"], axis=0),
        helper.make_node("Add", ["t", "one_i"], ["t1"]),
        helper.make_node("Range", ["one_i", "t1", "one_i"], ["positions"]),
        const("seq", np.array(SEQ + [2] * 600, dtype=np.int64)),
        helper.make_node("Gather", ["seq", "positions"], ["next"], axis=0),
        const("depth", np.array(V, dtype=np.int64)),
        const("values", np.array([0.0, 1.0], dtype=np.float32)),
        helper.make_node("OneHot", ["next", "depth", "values"], ["onehot"], axis=-1),
        const("axes0", np.array([0], dtype=np.int64)),
        helper.make_node("Unsqueeze", ["onehot", "axes0"], ["logits"]),
    ]
    out = helper.make_tensor_value_info("logits", TensorProto.FLOAT, [1, "t", V])
    return model(nodes, [x, mask, context], [out], "decoder")


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    save(resizer(), out / "image_resizer.onnx")
    save(encoder(), out / "encoder.onnx")
    save(decoder(), out / "decoder.onnx")
    vocab = {token: i for i, token in enumerate(VOCAB)}
    (out / "tokenizer.json").write_text(json.dumps({"version": "1.0", "model": {"type": "BPE", "vocab": vocab, "merges": []}}, indent=1))


if __name__ == "__main__":
    main()
