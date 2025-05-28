use core::cell::{Cell, RefCell, UnsafeCell};
use core::pin::Pin;
use core::ptr::NonNull;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub struct SansIO<T: ?Sized + Interface> {
    inner: <T as Sealed>::Repr,
}

pub struct Buffers<'lt> {
    pub input: &'lt [u8],
    pub output: &'lt mut [u8],
}

#[derive(Debug, Default)]
pub enum Demand {
    #[default]
    None,
    Consume(usize),
    Write(usize),
}

impl<T: ?Sized + Interface> SansIO<T> {
    pub fn step(this: Pin<&mut Self>, buffers: Buffers<'_>) -> Demand {
        let mut pass_buffers = UnsafeWaker {
            ctx: Cell::default(),
            state: RefCell::default(),
        };

        // We can *not* demand the underlying future to be Unpin, that contradicts the design of
        // IoFuture that already requires to be pinned. So any coroutine using it will also be. We
        // do not want everybody to pin this manually.
        let inner = &mut unsafe { this.get_unchecked_mut() }.inner;
        let inner = unsafe { Pin::new_unchecked(inner) };

        T::step(inner, &mut pass_buffers, buffers)
    }

    /// Consume the sized transformer.
    ///
    /// This pins it internally.
    pub fn with_io(
        mut self,
        input: &mut std::io::BufReader<dyn std::io::Read>,
        output: &mut dyn std::io::Write,
        obuf: &mut [u8],
    ) -> std::io::Result<()>
    where
        Self: Sized,
    {
        let this = core::pin::pin!(self);
        this.with_io_pinned(input, output, obuf)
    }

    /// Like [`Self::with_io`] that requires to be pinned, but also runs on unsized transformers.
    ///
    /// This way you can call it for `SansIO<dyn Future<Output = ()>>`.
    pub fn with_io_pinned(
        mut self: Pin<&mut Self>,
        input: &mut std::io::BufReader<dyn std::io::Read>,
        output: &mut dyn std::io::Write,
        obuf: &mut [u8],
    ) -> std::io::Result<()> {
        use std::io::BufRead;

        loop {
            let buffers = Buffers {
                input: input.fill_buf()?,
                output: obuf,
            };

            match SansIO::step(self.as_mut(), buffers) {
                Demand::None => return Ok(()),
                Demand::Consume(many) => {
                    input.consume(many);
                }
                Demand::Write(many) => {
                    output.write_all(&obuf[..many])?;
                }
            }
        }
    }
}

pub trait Interface: Sealed {}

impl<T: ?Sized> Interface for T where T: Sealed {}

use sealed::Sealed;

pub fn from_async<F>(mk: impl FnOnce(IoFuture) -> F) -> SansIO<F>
where
    F: Future<Output = ()>,
{
    let inner = mk(IoFuture {
        ctx: UnsafeCell::new(0u8),
        pinned: core::marker::PhantomPinned,
    });

    SansIO { inner }
}

pub struct IoFuture {
    // This is a placeholder so that we have a unique address together with pin, before this future
    // is being used.
    ctx: UnsafeCell<u8>,
    #[expect(dead_code)]
    pinned: core::marker::PhantomPinned,
}

pub struct GetBuffers<'lt>(Option<Pin<&'lt mut IoFuture>>);

pub struct Consume<'lt>(Pin<&'lt mut IoFuture>, Option<usize>);

pub struct Write<'lt>(Pin<&'lt mut IoFuture>, Option<usize>);

#[derive(Default)]
struct CommunicateWithWaker {
    // Set by the waker while polling.
    buffers: IoBuffers,
    // Set by us to signal the waker what the environment must do.
    demand: Demand,
}

impl IoFuture {
    pub fn get(self: Pin<&mut Self>) -> GetBuffers<'_> {
        GetBuffers(Some(self))
    }

    pub fn consume(self: Pin<&mut Self>, many: usize) -> Consume<'_> {
        Consume(self, Some(many))
    }

    pub fn write(self: Pin<&mut Self>, many: usize) -> Write<'_> {
        Write(self, Some(many))
    }
}

struct IoBuffers {
    input: NonNull<[u8]>,
    output: NonNull<[u8]>,
}

struct UnsafeWaker {
    // Yes, this is not `Sync`. But we can not clone the waker at *all* and in particular only pass
    // a reference to the actual waker when polling the future.
    ctx: Cell<Option<NonNull<u8>>>,
    state: RefCell<CommunicateWithWaker>,
}

impl Future for IoFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = UnsafeWaker::from_context(cx);
        let comm = waker.state.borrow_mut();

        // Ensure any demand is met first..
        if let Demand::None = comm.demand {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl<'lt> Future for GetBuffers<'lt> {
    type Output = Buffers<'lt>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = UnsafeWaker::from_context(cx);
        let inner = self.0.take().expect("already polled to completion");

        // Ensure we are in fact the only one interacting with this waker. This is necessary for
        // borrow checking. Below we extend the lifetime of the buffer borrow beyond the context.
        // To ensure that no two such borrows can occur, the IoFuture acts as a token type which
        // takes over the ownership, and then uses its own borrow checking to ensure exclusivity.
        let buffer = waker.borrow_mut(inner);

        // Always ready. We just do this to validate the buffers with the context.
        Poll::Ready(buffer)
    }
}

impl<'lt> Future for Consume<'lt> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = UnsafeWaker::from_context(cx);

        if let Some(demand) = self.1.take() {
            waker.state.borrow_mut().demand = Demand::Consume(demand);
        }

        self.0.as_mut().poll(cx)
    }
}

impl<'lt> Future for Write<'lt> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = UnsafeWaker::from_context(cx);

        if let Some(demand) = self.1.take() {
            waker.state.borrow_mut().demand = Demand::Write(demand);
        }

        self.0.as_mut().poll(cx)
    }
}

impl UnsafeWaker {
    const VTABLE: &'static RawWakerVTable =
        &RawWakerVTable::new(Self::clone, Self::wake, Self::wake_by_ref, Self::drop);

    // When cloning, we create a noop-waker. This technically wakes the same task but the waker is
    // no longer imbued with the special vtable. So if you ever call this, which only happens when
    // you manually persist it in your coroutine, you violate the contract and will get a runtime
    // error.
    unsafe fn clone(_: *const ()) -> RawWaker {
        const NOOP: &'static core::task::Waker = core::task::Waker::noop();
        RawWaker::new(NOOP.data(), NOOP.vtable())
    }

    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {
        assert!(
            !core::mem::needs_drop::<UnsafeWaker>(),
            "UnsafeWaker must not have drop glue"
        );
    }

    fn as_waker(&mut self) -> Waker {
        // Safety:
        // - This is the correct vtable for the creation.
        // - The vtable satisfies the contract.
        //   - clone creates a waker waking the same task (always the future inside `SansIO`).
        //   - wake, wake_by_ref, drop all do the right thing.
        //   - drop in particular is trivial as we do not have drop glue.
        unsafe { Waker::from_raw(RawWaker::new(self as *mut _ as *const (), Self::VTABLE)) }
    }

    fn take_demand(&mut self) -> Demand {
        core::mem::take(&mut self.state.borrow_mut().demand)
    }

    fn with_ctx<T>(&mut self, buffers: &mut Buffers<'_>, f: impl FnOnce(Context<'_>) -> T) -> T {
        let placeholder = {
            let buffers = IoBuffers {
                input: NonNull::from(&*buffers.input),
                output: NonNull::from(&mut *buffers.output),
            };

            // No buffer registered itself.. yet. so nothing is waiting on these.
            let mut comm = self.state.borrow_mut();
            // the ''return'' from the previous demand, so we reset.
            comm.demand = Demand::None;
            core::mem::replace(&mut comm.buffers, buffers)
        };

        let waker = self.as_waker();
        let result = f(Context::from_waker(&waker));

        {
            let mut comm = self.state.borrow_mut();
            comm.buffers = placeholder;
        }

        result
    }

    fn from_context<'a>(cx: &'a mut Context) -> &'a mut Self {
        let waker = cx.waker();

        assert!(
            waker.vtable() == UnsafeWaker::VTABLE,
            "This future must only be polled with the internal waker of `SansIO`"
        );

        let data = waker.data();
        // Safety: `IoBuffers` must only be polled via an `UnsafeWaker`.
        unsafe { &mut *(data as *mut UnsafeWaker) }
    }

    fn borrow_mut<'lt>(&self, pinned: Pin<&'lt mut IoFuture>) -> Buffers<'lt> {
        let token_ptr: NonNull<_> = NonNull::from(&pinned.as_ref().ctx);
        let token_ptr: NonNull<u8> = token_ptr.cast();
        let registered = self.ctx.get().unwrap_or(token_ptr);

        // Necessary for safety. This ensures the pinned future can be used to borrow check the
        // access below as the reference covers that pointer memory uniquely.
        assert!(token_ptr.as_ptr().addr() == registered.as_ptr().addr());
        let mut comm = self.state.borrow_mut();

        // Safety: We are the only one using this waker, so we can safely extend the lifetime of the
        // buffers.
        unsafe {
            Buffers {
                input: comm.buffers.input.as_ref(),
                output: comm.buffers.output.as_mut(),
            }
        }
    }
}

impl Default for IoBuffers {
    fn default() -> Self {
        IoBuffers {
            input: NonNull::from(&[]),
            output: NonNull::from(&mut []),
        }
    }
}

mod sealed {
    use super::*;

    pub trait Sealed {
        type Repr: ?Sized;

        #[expect(private_interfaces)]
        fn step(
            this: Pin<&mut Self::Repr>,
            waker: &mut UnsafeWaker,
            buffers: Buffers<'_>,
        ) -> Demand;
    }

    impl<F: Future<Output = ()> + ?Sized> Sealed for F {
        type Repr = F;

        #[expect(private_interfaces)]
        fn step(
            mut this: Pin<&mut Self>,
            comm: &mut UnsafeWaker,
            mut buffers: Buffers<'_>,
        ) -> Demand {
            loop {
                let this = this.as_mut();

                match comm.with_ctx(&mut buffers, |mut cx| this.poll(&mut cx)) {
                    Poll::Ready(()) => return Demand::None,
                    Poll::Pending => {}
                }

                // Does the environment need to do something?
                match comm.take_demand() {
                    Demand::None => {}
                    demand => return demand,
                }
            }
        }
    }
}
