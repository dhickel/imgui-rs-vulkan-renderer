use crate::{RendererError, RendererResult};
use ash::{vk, Device};
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc, Allocator},
    MemoryLocation,
};
use std::sync::{Arc, Mutex, MutexGuard};

use super::Allocate;

pub struct GpuAllocator {
    pub allocator: Arc<Mutex<Allocator>>,
}

impl GpuAllocator {
    fn get_allocator(&self) -> RendererResult<MutexGuard<Allocator>> {
        self.allocator.lock().map_err(|e| {
            RendererError::Allocator(format!(
                "Failed to acquire lock on allocator: {}",
                e.to_string()
            ))
        })
    }
}

impl Allocate for GpuAllocator {
    type Memory = Allocation;

    fn create_buffer(
        &mut self,
        device: &Device,
        size: usize,
        usage: vk::BufferUsageFlags,
    ) -> RendererResult<(vk::Buffer, Self::Memory)> {
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(size as _)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .build();

        let buffer = unsafe { device.create_buffer(&buffer_info, None)? };
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let mut allocator = self.get_allocator()?;

        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "",
            requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true,
        })?;

        unsafe { device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())? };

        Ok((buffer, allocation))
    }

    fn create_image(
        &mut self,
        device: &Device,
        width: u32,
        height: u32,
    ) -> RendererResult<(vk::Image, Self::Memory)> {
        let extent = vk::Extent3D {
            width,
            height,
            depth: 1,
        };

        let image_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(extent)
            .mip_levels(1)
            .array_layers(1)
            .format(vk::Format::R8G8B8A8_UNORM)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .flags(vk::ImageCreateFlags::empty());

        let image = unsafe { device.create_image(&image_info, None)? };
        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let mut allocator = self.get_allocator()?;

        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "",
            requirements,
            location: MemoryLocation::GpuOnly,
            linear: true,
        })?;

        unsafe { device.bind_image_memory(image, allocation.memory(), allocation.offset())? };

        Ok((image, allocation))
    }

    fn destroy_buffer(
        &mut self,
        device: &Device,
        buffer: vk::Buffer,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        let mut allocator = self.get_allocator()?;

        allocator.free(memory)?;
        unsafe { device.destroy_buffer(buffer, None) };

        Ok(())
    }

    fn destroy_image(
        &mut self,
        device: &Device,
        image: vk::Image,
        memory: Self::Memory,
    ) -> RendererResult<()> {
        let mut allocator = self.get_allocator()?;

        allocator.free(memory)?;
        unsafe { device.destroy_image(image, None) };

        Ok(())
    }

    fn update_buffer<T: Copy>(
        &mut self,
        _device: &Device,
        memory: &Self::Memory,
        data: &[T],
    ) -> RendererResult<()> {
        let size = (data.len() * std::mem::size_of::<T>()) as _;
        unsafe {
            let data_ptr = memory.mapped_ptr().unwrap().as_ptr();
            let mut align = ash::util::Align::new(data_ptr, std::mem::align_of::<T>() as _, size);
            align.copy_from_slice(data);
        };
        Ok(())
    }
}
