use sans_io::{IoFuture, from_async};

#[test]
fn decapitalize() {
    let transformer = from_async(|io_future: IoFuture| async move {
        let mut io_future = core::pin::pin!(io_future);
        io_future.as_mut().await;

        loop {
            let buffers = io_future.as_mut().get().await;

            if buffers.input.is_empty() {
                break;
            }

            let ilen = buffers.input.len();
            let olen = buffers.output.len();
            let len = ilen.min(olen);

            let iter = buffers.input[..ilen]
                .iter()
                .zip(&mut buffers.output[..olen]);

            for (ch, byte) in iter {
                *byte = ch.to_ascii_lowercase();
            }

            // Indicate how many bytes to write and consume them from the input.
            IoFuture::write(io_future.as_mut(), len).await;
            IoFuture::consume(io_future.as_mut(), len).await;
        }
    });

    let mut input = std::io::BufReader::new(std::io::Cursor::new(b"Hello, World!"));
    let mut obuf = vec![0; 4096];
    let mut output = vec![];

    transformer
        .with_io(&mut input, &mut output, &mut obuf[..])
        .unwrap();
    assert_eq!(output, b"hello, world!");
}
