use core::cell::{Cell, UnsafeCell};
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
            spin: 1,
        };

        let inner = &mut unsafe { this.get_unchecked_mut() }.inner;
        let inner = unsafe { Pin::new_unchecked(inner) };
        T::step(inner, &mut pass_buffers, buffers)
    }

    pub fn with_io<I: ?Sized + std::io::Read, O: ?Sized + std::io::Write>(
        mut this: Pin<&mut Self>,
        input: &mut std::io::BufReader<I>,
        output: &mut O,
        obuf: &mut [u8],
    ) -> std::io::Result<()> {
        use std::io::BufRead;

        loop {
            let buffers = Buffers {
                input: input.fill_buf()?,
                output: obuf,
            };

            match SansIO::step(this.as_mut(), buffers) {
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
        ctx: UnsafeCell::new(CommunicateWithWaker::default()),
    });
    SansIO { inner }
}

pub struct IoFuture {
    // FIXME: this must be `core::pin::UnsafePinned`! Otherwise each await on the future (which
    // gets the exact argument Pin<&mut Self>) will retag all of its attributes as unique. That is
    // we couldn't share a pointer to the attributes outside, that pointer is invalided immediately
    // on awaiting.
    ctx: UnsafeCell<CommunicateWithWaker>,
}

pub struct GetBuffers<'lt>(Pin<&'lt mut IoFuture>);

#[derive(Default)]
struct CommunicateWithWaker {
    // Set by the waker while polling.
    buffers: Option<IoBuffers>,
    // The buffers are valid while our spin equals the waker's.
    spin: u64,
    // Set by us to signal the waker what the environment must do.
    demand: Demand,
}

impl IoFuture {
    pub fn get(self: Pin<&mut Self>) -> GetBuffers<'_> {
        GetBuffers(self)
    }

    pub fn consume(mut this: Pin<&mut Self>, many: usize) -> Pin<&mut Self> {
        unsafe { this.as_mut().get_unchecked_mut() }
            .ctx
            .get_mut()
            .demand = Demand::Consume(many);
        this
    }

    pub fn write(mut this: Pin<&mut Self>, many: usize) -> Pin<&mut Self> {
        unsafe { this.as_mut().get_unchecked_mut() }
            .ctx
            .get_mut()
            .demand = Demand::Write(many);
        this
    }
}

struct IoBuffers {
    input: NonNull<[u8]>,
    output: NonNull<[u8]>,
}

struct UnsafeWaker {
    // Yes, this is not `Sync`. But we can not clone the waker at *all* and in particular only pass
    // a reference to the actual waker when polling the future.
    ctx: Cell<Option<BufferPtr>>,
    spin: u64,
}

type BufferPtr = NonNull<UnsafeCell<CommunicateWithWaker>>;

impl Future for IoFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = UnsafeWaker::from_context(cx);
        let this = Pin::get_ref(self.as_ref());

        {
            let comm = NonNull::from(&this.ctx);
            let registered = waker.ctx.get();

            assert!(
                registered.map_or(true, |b| b.as_ptr().addr() == comm.as_ptr().addr()),
                "You must not use a different io buffers"
            );

            // Safety: we are pinned. This being our special waker means we are being polled by
            // the library itself. We always share the latest pointer to ourselves even tough it's
            // address is equal to what we already have. The reason for this is provenance. When we
            // yield the caller should be able to await.
            waker.ctx.set(Some(comm));
        }

        let ctx = unsafe { &*this.ctx.get() };
        match ctx.buffers {
            Some(_) => {
                if let Demand::None = ctx.demand {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
            None => Poll::Pending,
        }
    }
}

impl<'lt> Future for GetBuffers<'lt> {
    type Output = Option<Buffers<'lt>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = UnsafeWaker::from_context(cx);

        let set_by_outside = &self.as_ref().0.ctx;
        let comm = unsafe { &mut *set_by_outside.get() };
        let Some(set_by_outside) = comm.buffers.as_mut() else {
            return Poll::Ready(None);
        };

        if waker.spin != comm.spin {
            return Poll::Ready(None);
        }

        // Always ready. We just do this to validate the buffers with the context.
        Poll::Ready(Some(Buffers {
            input: unsafe { set_by_outside.input.as_ref() },
            output: unsafe { set_by_outside.output.as_mut() },
        }))
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
        match self.ctx.get() {
            // No buffer registered itself.. yet.
            None => Demand::None,
            Some(comm) => {
                let comm = unsafe { &mut *(comm.as_ref().get()) };
                core::mem::take(&mut comm.demand)
            }
        }
    }

    fn with_ctx<T>(&mut self, buffers: &mut Buffers<'_>, f: impl FnOnce(Context<'_>) -> T) -> T {
        let buffers = IoBuffers {
            input: NonNull::from(&*buffers.input),
            output: NonNull::from(&mut *buffers.output),
        };

        match self.ctx.get() {
            // No buffer registered itself.. yet. so nothing is waiting on these.
            None => {}
            Some(comm) => {
                let comm = unsafe { &mut *(comm.as_ref().get()) };
                comm.buffers = Some(buffers);
                comm.spin = self.spin;
            }
        };

        let waker = self.as_waker();
        let cx = Context::from_waker(&waker);

        let result = f(cx);

        // This is a little bit early to remove the buffers, if we do not have a demand they can be
        // re-used. But that is a few instructions which isn't really important.
        //
        // We _must not_ use the pointer at this point. The coroutine has yielded but here we can
        // not be sure it did so through awaiting on our pin. It could have internally ran
        // `Pin::as_mut` or similar which retags all the IoFuture attributes as unique. That
        // includes invalidating the pointer that the future shared to us!
        //
        // Nevertheless we must invalidate the buffers. More specifically the pointers must not be
        // dereferenced after we return from this function. To do both, we require that access to
        // the buffers *also* gets a reference to the Waker and then do a counting scheme.
        self.spin += 1;

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

    impl<F: Future<Output = ()>> Sealed for F {
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
