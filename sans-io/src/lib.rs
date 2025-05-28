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
    ctx: UnsafeCell<CommunicateWithWaker>,
}

#[derive(Default)]
struct CommunicateWithWaker {
    // Set by the waker while polling.
    buffers: Option<IoBuffers>,
    // Set by us to signal the waker what the environment must do.
    demand: Demand,
}

impl IoFuture {
    pub fn get<'a>(this: Pin<&'a mut Self>) -> Option<Buffers<'a>> {
        let set_by_outside = unsafe { this.get_unchecked_mut() }
            .ctx
            .get_mut()
            .buffers
            .as_mut()?;

        Some(Buffers {
            input: unsafe { set_by_outside.input.as_ref() },
            output: unsafe { set_by_outside.output.as_mut() },
        })
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
}

type BufferPtr = NonNull<CommunicateWithWaker>;

impl Future for IoFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = cx.waker();

        assert!(
            waker.vtable() == UnsafeWaker::VTABLE,
            "This future must only be polled with the internal waker of `SansIO`"
        );

        let data = waker.data();
        // Safety: `IoBuffers` must only be polled via an `UnsafeWaker`.
        let waker = unsafe { &mut *(data as *mut UnsafeWaker) };
        let this = unsafe { self.get_unchecked_mut() };

        {
            let comm = NonNull::new(this.ctx.get()).unwrap();
            let registered = waker.ctx.get();

            assert!(
                registered.map_or(true, |b| b.as_ptr() == comm.as_ptr()),
                "You must not use a different io buffers"
            );

            // Safety: we are pinned. This being our special waker means we are being polled by
            // the library itself.
            if registered.is_none() {
                waker.ctx.set(Some(comm));
            }
        }

        match this.ctx.get_mut().buffers {
            Some(_) => {
                if let Demand::None = this.ctx.get_mut().demand {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
            None => {
                Poll::Pending
            }
        }
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
        unsafe { Waker::from_raw(RawWaker::new(self as *const _ as *const (), Self::VTABLE)) }
    }

    fn take_demand(&mut self) -> Demand {
        match self.ctx.get() {
            // No buffer registered itself.. yet.
            None => Demand::None,
            Some(mut comm) => {
                let comm = unsafe { comm.as_mut() };
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
            Some(mut comm) => {
                let comm = unsafe { comm.as_mut() };
                comm.buffers = Some(buffers);
            }
        };

        let waker = self.as_waker();
        let cx = Context::from_waker(&waker);

        let result = f(cx);

        // This is a little bit early to remove the buffers, if we do not have a demand they can be
        // re-used. But that is a few instructions which isn't really important.
        if let Some(mut comm) = self.ctx.get() {
            let comm = unsafe { comm.as_mut() };
            comm.buffers = None;
        }

        result
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
