use common::utils::calculate_hash;
use network::messages::ResurceScheme;
use std::io::Write;
use zip::{write::SimpleFileOptions, DateTime};

pub const ARCHIVE_CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Default)]
pub struct ResourcesArchive {
    archive_data: Vec<u8>,
    archive_hash: Option<u64>,
    entries: Vec<(String, Vec<u8>)>,

    resources_scheme: Vec<ResurceScheme>,
}

impl ResourcesArchive {
    pub fn add_entry(&mut self, name: impl Into<String>, data: Vec<u8>) {
        self.entries.push((name.into(), data));
    }

    pub fn add_resource_scheme(&mut self, scheme: ResurceScheme) {
        self.resources_scheme.push(scheme);
    }

    pub fn get_resources_scheme(&self) -> &Vec<ResurceScheme> {
        &self.resources_scheme
    }

    pub fn has_any(&self) -> bool {
        self.get_archive_hash();
        self.archive_data.len() > 0
    }

    pub fn finalize(&mut self) {
        let mut archive_data: Vec<u8> = Vec::new();
        let buff = std::io::Cursor::new(&mut archive_data);
        let mut writer = zip::ZipWriter::new(buff);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .last_modified_time(DateTime::default());

        for (name, data) in self.entries.drain(..) {
            writer.start_file(name.as_str(), options).unwrap();
            writer.write_all(&data).unwrap();
        }

        writer.finish().unwrap();

        let hash = calculate_hash(&archive_data);
        self.archive_hash = Some(hash);
        self.archive_data = archive_data;
    }

    pub fn get_archive_hash(&self) -> u64 {
        self.archive_hash.expect("archive not finalized")
    }

    pub fn get_archive_len(&self) -> usize {
        self.get_archive_hash();
        self.archive_data.len()
    }

    pub fn get_archive_parts_count(&self, chunk_size: usize) -> usize {
        self.get_archive_hash();
        self.archive_data.len().div_ceil(chunk_size)
    }

    pub fn get_archive_part(&self, index: usize, chunk_size: usize) -> Vec<u8> {
        self.get_archive_hash();

        let parts_count = self.get_archive_parts_count(chunk_size);
        assert!(
            index < parts_count,
            "archive chunk index:{} must be less than max:{}",
            index,
            parts_count
        );

        let start = index * chunk_size;

        let mut end = (index + 1) * chunk_size;
        end = self.get_archive_len().min(end);

        let slice = &self.archive_data[start..end];
        slice.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use crate::plugins::resources_archive::{ResourcesArchive, ARCHIVE_CHUNK_SIZE};
    use common::utils::calculate_hash;
    use std::io::Read;

    #[test]
    fn test_archive() {
        let mut resources_archive = ResourcesArchive::default();

        let file_content = "content".to_string().into_bytes();
        let hash = calculate_hash(&file_content);
        resources_archive.add_entry(hash.to_string(), file_content.clone());
        resources_archive.finalize();

        assert_eq!(resources_archive.archive_hash.unwrap(), 431488420107704094);

        let data = [
            80, 75, 3, 4, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 169, 48, 197, 254, 7, 0, 0, 0, 7, 0, 0, 0, 19, 0, 0, 0, 52,
            52, 56, 57, 56, 49, 54, 50, 48, 57, 48, 48, 56, 50, 51, 52, 49, 57, 57, 99, 111, 110, 116, 101, 110, 116,
            80, 75, 1, 2, 10, 3, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 169, 48, 197, 254, 7, 0, 0, 0, 7, 0, 0, 0, 19, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 129, 0, 0, 0, 0, 52, 52, 56, 57, 56, 49, 54, 50, 48, 57, 48, 48, 56, 50,
            51, 52, 49, 57, 57, 80, 75, 5, 6, 0, 0, 0, 0, 1, 0, 1, 0, 65, 0, 0, 0, 56, 0, 0, 0, 0, 0,
        ];
        assert_eq!(*resources_archive.archive_data, data);
        assert_eq!(resources_archive.get_archive_len(), 143);
        assert_eq!(resources_archive.get_archive_parts_count(50), 3);
        assert_eq!(
            resources_archive.get_archive_part(0, 50),
            [
                80, 75, 3, 4, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 169, 48, 197, 254, 7, 0, 0, 0, 7, 0, 0, 0, 19, 0, 0, 0,
                52, 52, 56, 57, 56, 49, 54, 50, 48, 57, 48, 48, 56, 50, 51, 52, 49, 57, 57, 99
            ]
        );
        assert_eq!(
            resources_archive.get_archive_part(1, 50),
            [
                111, 110, 116, 101, 110, 116, 80, 75, 1, 2, 10, 3, 10, 0, 0, 0, 0, 0, 0, 0, 33, 0, 169, 48, 197, 254,
                7, 0, 0, 0, 7, 0, 0, 0, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 164, 129, 0, 0
            ]
        );
        assert_eq!(
            resources_archive.get_archive_part(2, 50),
            [
                0, 0, 52, 52, 56, 57, 56, 49, 54, 50, 48, 57, 48, 48, 56, 50, 51, 52, 49, 57, 57, 80, 75, 5, 6, 0, 0,
                0, 0, 1, 0, 1, 0, 65, 0, 0, 0, 56, 0, 0, 0, 0, 0
            ]
        );

        let chunk = resources_archive.get_archive_part(0, ARCHIVE_CHUNK_SIZE);
        assert_eq!(resources_archive.get_archive_hash(), calculate_hash(&chunk));

        let archive_data = resources_archive.archive_data;
        let file = std::io::Cursor::new(&archive_data);

        let mut zip = zip::ZipArchive::new(file).unwrap();
        for i in 0..zip.len() {
            let archive_file = zip.by_index(i).unwrap();
            let file_hash = archive_file.name().to_string();

            let mut archive_file_data = Vec::new();
            for i in archive_file.bytes() {
                archive_file_data.push(i.unwrap());
            }

            assert_eq!(archive_file_data, file_content);

            let hash = calculate_hash(&archive_file_data);
            assert_eq!(hash.to_string(), file_hash);
        }
    }
}
