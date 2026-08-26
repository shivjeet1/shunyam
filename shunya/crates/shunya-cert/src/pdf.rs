use crate::WipeManifest;
use lopdf::dictionary;
use lopdf::{Document, Object, Dictionary, StringFormat};
use qrcode::QrCode;
use image::{Luma, ImageBuffer};

pub struct PdfGenerator;

impl PdfGenerator {
    pub fn generate(manifest: &WipeManifest, signature: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let mut doc = Document::with_version("1.5");
        
        // In a full implementation, we'd build the Page tree, content streams (Text),
        // and XObjects (for the QR image and signatures).
        // Lopdf is quite low-level, so building a full page involves creating /Catalog, /Pages, /Page, etc.
        
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let content = format!(
            "BT\n\
            /F1 12 Tf\n\
            50 750 Td\n\
            (Shunya Secure Wipe Certificate) Tj\n\
            0 -20 Td\n\
            (Job ID: {}) Tj\n\
            0 -20 Td\n\
            (Device Model: {}) Tj\n\
            0 -20 Td\n\
            (Device Serial: {}) Tj\n\
            0 -20 Td\n\
            (Method: {}) Tj\n\
            0 -20 Td\n\
            (Manifest Hash: {}) Tj\n\
            ET",
            manifest.job_id,
            manifest.device_model,
            manifest.device_serial,
            manifest.wipe_method,
            manifest.hash()
        );

        let content_id = doc.add_object(Object::Stream(lopdf::Stream::new(
            Dictionary::new(),
            content.as_bytes().to_vec(),
        )));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                }
            },
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };

        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });

        doc.trailer.set("Root", catalog_id);

        let mut out = Vec::new();
        doc.save_to(&mut out).map_err(|e| e.to_string())?;

        Ok(out)
    }

    pub fn generate_qr(manifest: &WipeManifest) -> Result<Vec<u8>, String> {
        let code = QrCode::new(manifest.canonical_json().as_bytes())
            .map_err(|e| e.to_string())?;
            
        let image: ImageBuffer<Luma<u8>, Vec<u8>> = code.render::<Luma<u8>>().build();
        
        let mut buf = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut buf,
            &image,
            image.width(),
            image.height(),
            image::ColorType::L8,
            image::ImageFormat::Png,
        ).map_err(|e| e.to_string())?;

        Ok(buf.into_inner())
    }
}
