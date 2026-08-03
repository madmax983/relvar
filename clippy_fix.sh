#!/bin/bash
for f in relvar/src/experimental/*.rs; do
  sed -i '1i #![allow(dead_code)]' "$f"
done
