use super::hresult_error::*;
use super::window::Window;
use maboy::MemPixel;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr;
use winapi::shared::dxgi::*;
use winapi::shared::dxgiformat::*;
use winapi::shared::minwindef::*;
use winapi::shared::winerror::*;
use winapi::shared::{dxgi1_2::*, dxgitype::*};
use winapi::um::d3d11::*;
use winapi::um::d3dcommon::*;
use winapi::um::unknwnbase::IUnknown;
use winapi::Interface;
use wio::com::ComPtr;

pub struct GfxDevice {
    d: ComPtr<ID3D11Device>,
    dc: ComPtr<ID3D11DeviceContext>,
    dxgi_factory: ComPtr<IDXGIFactory2>,
}

impl GfxDevice {
    pub fn new() -> Result<GfxDevice, HResultError> {
        unsafe {
            let mut flags = D3D11_CREATE_DEVICE_SINGLETHREADED; // TODO: Think about this

            if cfg!(debug_assertions) {
                flags |= D3D11_CREATE_DEVICE_DEBUG;
            }

            // Create device

            let mut d = ptr::null_mut();
            let mut dc = ptr::null_mut();
            D3D11CreateDevice(
                ptr::null_mut(),
                D3D_DRIVER_TYPE_HARDWARE,
                ptr::null_mut(),
                flags,
                ptr::null(),
                0,
                D3D11_SDK_VERSION,
                &mut d,
                ptr::null_mut(),
                &mut dc,
            )
            .into_result()?;
            let d = ComPtr::from_raw(d);
            let dc = ComPtr::from_raw(dc);

            // Cast to DXGI device

            let mut dxgi_device = ptr::null_mut();
            d.QueryInterface(&IDXGIDevice2::uuidof(), &mut dxgi_device)
                .into_result()?;
            let dxgi_device = ComPtr::from_raw(dxgi_device as *mut IDXGIDevice2);

            // Extracty DXGI Adapter

            let mut dxgi_adapter = ptr::null_mut();
            dxgi_device.GetAdapter(&mut dxgi_adapter).into_result()?;
            let dxgi_adapter = ComPtr::from_raw(dxgi_adapter as *mut IDXGIAdapter2);

            // Extract DXGI Factory

            let mut dxgi_factory = ptr::null_mut();
            dxgi_adapter
                .GetParent(&IDXGIFactory2::uuidof(), &mut dxgi_factory)
                .into_result()?;
            let dxgi_factory = ComPtr::from_raw(dxgi_factory as *mut IDXGIFactory2);

            Ok(GfxDevice {
                d,
                dc,
                dxgi_factory,
            })
        }
    }

    pub fn create_gfx_window<I: Into<Option<u32>>>(
        &self,
        window: &Pin<Box<Window>>,
        width: I,
        height: I,
    ) -> Result<GfxWindow, HResultError> {
        unsafe {
            // Create swap-chain

            let scd = DXGI_SWAP_CHAIN_DESC1 {
                Width: width.into().unwrap_or(0),
                Height: height.into().unwrap_or(0),
                // For a flip-model swap chain (that is, a swap chain that has the DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL value set in the SwapEffect member), you must set the Format member to DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_B8G8R8A8_UNORM, or DXGI_FORMAT_R8G8B8A8_UNORM;
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                Stereo: FALSE,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_UNSPECIFIED,
                Flags: DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING,
            };

            let mut swap_chain = ptr::null_mut();
            self.dxgi_factory
                .CreateSwapChainForHwnd(
                    self.d.as_raw() as *mut IUnknown,
                    window.hwnd(),
                    &scd,
                    ptr::null(),
                    ptr::null_mut(),
                    &mut swap_chain,
                )
                .into_result()?;
            let swap_chain = ComPtr::from_raw(swap_chain); //IDXGISwapChain1

            // Get backbuffer from swap-chain

            let mut backbuffer = ptr::null_mut();
            swap_chain
                .GetBuffer(0, &ID3D11Texture2D::uuidof(), &mut backbuffer)
                .into_result()?;
            let backbuffer = ComPtr::from_raw(backbuffer as *mut ID3D11Texture2D);

            let mut backbuffer_desc: D3D11_TEXTURE2D_DESC = MaybeUninit::zeroed().assume_init();
            backbuffer.GetDesc(&mut backbuffer_desc);

            // Create viewport from backbuffer dimensions

            let mut viewport: D3D11_VIEWPORT = MaybeUninit::zeroed().assume_init();
            viewport.Height = backbuffer_desc.Height as f32;
            viewport.Width = backbuffer_desc.Width as f32;
            viewport.MinDepth = 0.0;
            viewport.MaxDepth = 1.0;

            // Create RTV for backbuffer
            let mut backbuffer_rtv = ptr::null_mut();
            self.d
                .CreateRenderTargetView(
                    backbuffer.as_raw() as *mut ID3D11Resource, // TODO: .up::<ID3D11Resource>() ???
                    ptr::null(),
                    &mut backbuffer_rtv,
                )
                .into_result()?;
            let backbuffer_rtv = ComPtr::from_raw(backbuffer_rtv);

            Ok(GfxWindow {
                device_context: self.dc.clone(),
                swap_chain,
                backbuffer,
                backbuffer_rtv,
                viewport,
                _window: PhantomData,
            })
        }
    }
}

pub struct GfxWindow<'w> {
    device_context: ComPtr<ID3D11DeviceContext>,
    swap_chain: ComPtr<IDXGISwapChain1>,
    backbuffer: ComPtr<ID3D11Texture2D>,
    backbuffer_rtv: ComPtr<ID3D11RenderTargetView>,
    viewport: D3D11_VIEWPORT,
    _window: PhantomData<&'w ()>,
}

impl<'w> GfxWindow<'w> {
    pub fn next_frame(&mut self) -> GfxFrame<'_, 'w> {
        // Note: Seems like we don't need this stuff. I'll leave it out for now

        // Might need to set depth-stencil in here at some point
        // self.device
        //     .dc
        //     .OMSetRenderTargets(1, &self.backbuffer_rtv.as_raw(), ptr::null_mut());

        // self.device.dc.RSSetViewports(1, &self.viewport);

        GfxFrame(self)
    }
}

pub struct GfxFrame<'a, 'w>(&'a mut GfxWindow<'w>);

impl GfxFrame<'_, '_> {
    pub fn clear(&mut self, color: &[f32; 4]) {
        unsafe {
            // Might need this someday:
            // m_d3dContext->ClearDepthStencilView(m_depthStencilView.Get(),
            //     D3D11_CLEAR_DEPTH | D3D11_CLEAR_STENCIL, 1.0f, 0);

            // Appararently, on Xbox One, this needs to go BEFORE OMSetRenderTargets: https://github.com/microsoft/DirectXTK/wiki/The-basic-game-loop
            self.0
                .device_context
                .ClearRenderTargetView(self.0.backbuffer_rtv.as_raw(), color);
        }
    }

    pub fn copy_from_slice(&mut self, data: &[MemPixel]) {
        unsafe {
            assert_eq!(
                data.len(),
                self.0.viewport.Width as usize * self.0.viewport.Height as usize,
                "Slice does not have the exact number of pixels that the window backbuffer requires"
            );

            self.0.device_context.UpdateSubresource(
                self.0.backbuffer.as_raw() as *mut ID3D11Resource,
                0,
                ptr::null(),
                data as *const _ as *const std::ffi::c_void,
                self.0.viewport.Width as u32 * 4,
                0,
            );
        }
    }

    pub fn present(self, blocking: bool) -> Result<(), HResultError> {
        unsafe {
            // TODO: Read up on whatever sync intervals are for DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL
            // TODO: Think about DXGI_PRESENT_DO_NOT_WAIT
            // TODO: Really read up on the tearing docs at https://docs.microsoft.com/en-us/windows/win32/direct3ddxgi/dxgi-present

            let (sync_interval, flags) = if blocking {
                (1, 0)
            } else {
                (0, DXGI_PRESENT_ALLOW_TEARING)
            };

            let result = self
                .0
                .swap_chain
                .Present(sync_interval, flags)
                .into_result();

            if matches!(result, Err(HResultError(DXGI_ERROR_WAS_STILL_DRAWING))) {
                return Ok(());
            } else {
                result
            }
        }
    }
}
