A collection of crates related to `io` that is beyond the scope of Rust's
`std::io` functionality.

<dl>
  <dt>flexible-io</dt><dd>

  Make internal use of _optional_ IO capabilities such as `read_at` on Unix or
  `Seek` without cfg-hell in your interfaces. This provides trait bounds as a
  runtime choice, decoupling the use of the these trait-based interfaces from
  the scope in which a caller proved them.

  </dd>
  <dt>not-io</dt><dd>

  Fully SemVer compatible `no_std` `io` traits for slices etc. where a feature
  opts-in to the impl in terms of standard IO traits under the hood. SemVer
  compatibility extends to adding this feature, sibling crates can not break
  your code.

  </dd>
</dl>
