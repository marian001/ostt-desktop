//! Pinned model catalogue, generated from upstream `ModelCatalog.java` / `DefaultModelContract.java`
//! (offline-speech-to-text @ 5cf11c5). Every entry is an exact Hugging Face revision with its byte
//! size and SHA-256; a download is only activated when both match.

pub struct Entry {
    pub family: &'static str,
    pub repo: &'static str,
    pub revision: &'static str,
    pub file: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

pub const DEFAULT_FILE: &str = "parakeet-tdt-0.6b-v3-Q4_K_M.gguf";

/// Language detector (VoxLingua107 ECAPA-TDNN); auxiliary, never selectable as the speech model.
pub const LANGUAGE_ID: Entry = Entry {
    family: "ECAPA-TDNN",
    repo: "lang-id-voxlingua107-ecapa-gguf",
    revision: "e59f0f3847e77723b7d58447ef9456fa0c7e9436",
    file: "lang-id-voxlingua107-ecapa-Q8_0.gguf",
    bytes: 26693632,
    sha256: "063e6cb0af3d53085fc7d68356dfde5ecee2ffa7bc79855f7e4ff930081bfe3a",
};

pub const MODELS: &[Entry] = &[
    Entry { family: "Parakeet", repo: "parakeet-tdt-0.6b-v3-gguf", revision: "85ac09ea12fc4b1112fa76810059364bc6adc9de", file: "parakeet-tdt-0.6b-v3-Q4_K_M.gguf", bytes: 485425504, sha256: "b68557be1e3c40207fd7c4bd9d63f1d3316b963f15325bfb0cc16a8bb0ffd181" },
    Entry { family: "Parakeet", repo: "parakeet-tdt-0.6b-v3-gguf", revision: "d8d8c32cb2017c57e5307767343e97870f8a7f53", file: "parakeet-tdt-0.6b-v3-Q8_0.gguf", bytes: 739508576, sha256: "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7" },
    Entry { family: "Parakeet", repo: "parakeet-primeline-gguf", revision: "7c63545b352e5d410ccc1ea1eb49aa751e65780b", file: "parakeet-primeline-Q4_K_M.gguf", bytes: 485425568, sha256: "8b01f1893fa1db166d8b08375f8300c140fc7f110af7e9b68480a27b145dad7d" },
    Entry { family: "Parakeet", repo: "parakeet-ultra-gguf", revision: "a1539be1b849ea7463b88343e5277b373d0c4ea9", file: "parakeet-ultra-Q4_K_M.gguf", bytes: 486280096, sha256: "c69b7a5f9071a7afd8a1818ef7ae542c386c4b01ed0cac380c19a99947646bf9" },
    Entry { family: "Canary", repo: "canary-180m-flash-gguf", revision: "456e6049062ecf06f1a0f4607f2ee3dc80ebbf8a", file: "canary-180m-flash-Q4_K_M.gguf", bytes: 139223744, sha256: "c8ae5758d7d4dc59c48d816474a33544e362ca52c635f96ee0b4b95e1b48f90c" },
    Entry { family: "Canary", repo: "canary-1b-flash-gguf", revision: "ea436b4968b6a4e5b34fbe9a51a961388f1bda71", file: "canary-1b-flash-Q4_K_M.gguf", bytes: 677141280, sha256: "2521a615a2b04ab11900d894e1f5c5a70405d85aa74be0806b569b9cc311a707" },
    Entry { family: "Canary", repo: "canary-1b-gguf", revision: "a44e9c7596fe5e37040bbad8c54fba47f14c992f", file: "canary-1b-Q4_K_M.gguf", bytes: 729686848, sha256: "ded40db7f9d25e992bf35a0c3c3743e1a7c85d0c30bdbbabd46bca932a47353d" },
    Entry { family: "Canary", repo: "canary-1b-v2-gguf", revision: "692af661b140183b60e44f55ad03c59e1e77d078", file: "canary-1b-v2-Q4_K_M.gguf", bytes: 735476448, sha256: "49e0a67e219bec95a254c2348460b6350e75a7ac6f93a131e48244b4c7cb53b9" },
    Entry { family: "Canary", repo: "canary-qwen-2.5b-gguf", revision: "a4480da236b7d99d7a12186868de021ef4ff5ced", file: "canary-qwen-2.5b-Q4_K_M.gguf", bytes: 1737575808, sha256: "db5162229d6fa22597d06a613bd9b543eddb3ee02e6afc5e759120fde02bebf7" },
    Entry { family: "Cohere", repo: "cohere-transcribe-03-2026-gguf", revision: "7353b6040bdc1be9e5a3823d99d5fe12afe88264", file: "cohere-transcribe-03-2026-Q4_K_M.gguf", bytes: 1558162944, sha256: "0ea56826d8bd5d74b7143a4a04e022dc1bb75452cfae49d98b6acb0c1d16a1fb" },
    Entry { family: "Fun-ASR", repo: "Fun-ASR-MLT-Nano-2512-gguf", revision: "5db7c5bcebbc70d6c372e825399c3019f6089af7", file: "Fun-ASR-MLT-Nano-2512-Q4_K_M.gguf", bytes: 556975168, sha256: "9232584a27a7dcbcaf13640de8f9c2e7375178ea11e587f90a5328ecf06d58a1" },
    Entry { family: "Fun-ASR", repo: "Fun-ASR-Nano-2512-gguf", revision: "b0637e253b8805ce23e7e815a6673bed7ed6914b", file: "Fun-ASR-Nano-2512-Q4_K_M.gguf", bytes: 556974848, sha256: "444885fb7a16211a721e265dfe08c31b31e81fb9e2c4f21ab1079210cd038408" },
    Entry { family: "GigaAM", repo: "gigaam-v3-ctc-gguf", revision: "53ac479633748119e11b049cf28bc801f0440f10", file: "gigaam-v3-ctc-Q4_K_M.gguf", bytes: 182150080, sha256: "16b329fed3eaa197f2191506ac3cad72f46502297ed6a0f915c79d2ac08c404e" },
    Entry { family: "GigaAM", repo: "gigaam-v3-e2e-ctc-gguf", revision: "59717de63d50dd9dd93b3249da6ad3f2f1c4ad6b", file: "gigaam-v3-e2e-ctc-Q4_K_M.gguf", bytes: 182497888, sha256: "35581e11e048eef785657cff07e5fced794bba6d9c75143257f6452c8aeea655" },
    Entry { family: "GigaAM", repo: "gigaam-v3-e2e-rnnt-gguf", revision: "3efd5b39882ceadca5570a8fb9fa1a2c801d6d6d", file: "gigaam-v3-e2e-rnnt-Q4_K_M.gguf", bytes: 183948704, sha256: "7d69952fb431a8d7800ed9910dc61fea37d8406bfe96d10bf24c8bd4b7c68623" },
    Entry { family: "GigaAM", repo: "gigaam-v3-rnnt-gguf", revision: "93cba3cdf21d66ca7a746718c849419c2946d6bb", file: "gigaam-v3-rnnt-Q4_K_M.gguf", bytes: 183246752, sha256: "33b09558330ccfdc5c156e6d6b57c89759a0ac3b653bcdfcd5f1e13f20fc1e76" },
    Entry { family: "Granite", repo: "granite-4.0-1b-speech-gguf", revision: "9f243e662e448e8bcc64c3b1195413e106002334", file: "granite-4.0-1b-speech-Q4_K_M.gguf", bytes: 1602904800, sha256: "eacd9bc06063a3fdb6bf904a2a82c2646d504922c6e4e5eb1988d61e780884d1" },
    Entry { family: "Granite", repo: "granite-speech-4.1-2b-gguf", revision: "8636dae5eebb011a3d9b56215944ed0925a0d035", file: "granite-speech-4.1-2b-Q4_K_M.gguf", bytes: 1602904800, sha256: "3171de08a99315481f0eac1979cbc0684911631d59907b2fe1e189dbcdce135f" },
    Entry { family: "Granite", repo: "granite-speech-4.1-2b-nar-gguf", revision: "97fd42ae95532f13f119f78e048f46b5f5f1eee3", file: "granite-speech-4.1-2b-nar-Q4_K_M.gguf", bytes: 1560008832, sha256: "68537a51d4d6d5a6be6e50f735a08af16646afba70fe8ab80dfe60ba7623325f" },
    Entry { family: "Granite", repo: "granite-speech-4.1-2b-plus-gguf", revision: "fc98d5c650f77eaa6210f0d70aac73c3c567f38c", file: "granite-speech-4.1-2b-plus-Q4_K_M.gguf", bytes: 1489663424, sha256: "5d52b6bb9bc2b504120448b4da61aeb383da3c1bcfe88b22c6ed667f2520206f" },
    Entry { family: "MedASR", repo: "medasr-gguf", revision: "f17cb2ed8e1fe7943a3108b2fa43b289893b4427", file: "medasr-Q4_K_M.gguf", bytes: 83082432, sha256: "b23b52db7e1e17a5a21c24f3b78add04e1055bbe026b7c7196324c34ab6f3e2f" },
    Entry { family: "Moonshine", repo: "moonshine-base-gguf", revision: "124f8cf8d3c1c56ad0f6ebc44aaafc0622e62d97", file: "moonshine-base-Q8_0.gguf", bytes: 77476480, sha256: "7f0027dfd857d310b63a85ef57cadf183da712cc374f85a648f8bc18aaa2efc8" },
    Entry { family: "Moonshine", repo: "moonshine-streaming-medium-gguf", revision: "56302cde2ae162d1357a833d91775bcd06e91e1b", file: "moonshine-streaming-medium-Q8_0.gguf", bytes: 295793568, sha256: "f7c9564249b508f6012927ec4f9e536087da53a7047f858ca9975bea5f75299e" },
    Entry { family: "Moonshine", repo: "moonshine-streaming-small-gguf", revision: "e53f28b0cb90308390c852068b950a1aadd238b0", file: "moonshine-streaming-small-Q8_0.gguf", bytes: 198506848, sha256: "d03670f69629b649085d0f44a63d97668b4119117cc9611a4e4ad94341713dfc" },
    Entry { family: "Moonshine", repo: "moonshine-streaming-tiny-gguf", revision: "4125c28c22a76494162094ab3566387cd92fb2f5", file: "moonshine-streaming-tiny-Q8_0.gguf", bytes: 50462816, sha256: "930e4622ad3a24158b91406c30c977fa6a26b34cb32d6ac3e57cfb23383a869e" },
    Entry { family: "Moonshine", repo: "moonshine-tiny-gguf", revision: "c71a93b8f1cb2b7f7336f31bb54bc96960c16f41", file: "moonshine-tiny-Q8_0.gguf", bytes: 35466912, sha256: "2fd348d7b38f97d309cc3ec6848f3f57f537b80244950f07d2637e463f95a3a1" },
    Entry { family: "Nemotron", repo: "nemotron-3.5-asr-streaming-0.6b-gguf", revision: "53ce288f69326b415aeef030cfcb02a08a8096dd", file: "nemotron-3.5-asr-streaming-0.6b-Q4_K_M.gguf", bytes: 495831520, sha256: "41c99fa5fb6f3d35f68e79adc3e755eca2232a8d921178bd647b71194792b8fd" },
    Entry { family: "Nemotron", repo: "nemotron-speech-streaming-en-0.6b-gguf", revision: "7df420cc3fb915a3b334560dac712af44fc41ef5", file: "nemotron-speech-streaming-en-0.6b-Q4_K_M.gguf", bytes: 475436032, sha256: "dc959ca31499b114e395c44eb4f0778968f20e5cfb03305a08a39925b2da8e1e" },
    Entry { family: "Parakeet", repo: "parakeet-ctc-0.6b-gguf", revision: "437fa3bded0ca9bb67cae801964396c913256c16", file: "parakeet-ctc-0.6b-Q4_K_M.gguf", bytes: 469302464, sha256: "2a0dc545d06184b30cd3441a38811f09867a1d6b7b89f008f16bd75c1030ccdb" },
    Entry { family: "Parakeet", repo: "parakeet-ctc-1.1b-gguf", revision: "93c267c7c5c97dddc7ce98648b086422419e948b", file: "parakeet-ctc-1.1b-Q4_K_M.gguf", bytes: 818156576, sha256: "9e4c104086067ee3afefbf7d41929024261682926cf0a57cc5a1f557fc5b69e5" },
    Entry { family: "Parakeet", repo: "parakeet-rnnt-0.6b-gguf", revision: "3373d1aeee5a5257c94c2fa6c4b12302ce7be2aa", file: "parakeet-rnnt-0.6b-Q4_K_M.gguf", bytes: 476390816, sha256: "70b32229e2b576a3be66176ebc484476f3d715d911c11a4e17167790ce378cf7" },
    Entry { family: "Parakeet", repo: "parakeet-rnnt-1.1b-gguf", revision: "ed23dc01082573cc3d220a4bf12f31797f55eaf6", file: "parakeet-rnnt-1.1b-Q4_K_M.gguf", bytes: 825244928, sha256: "9e8636f491867751ca5b5c18ca91adf52014b1ae406b46516a9a8680ba1049a1" },
    Entry { family: "Parakeet", repo: "parakeet-tdt-0.6b-v2-gguf", revision: "58239cf6d0ecb0af921e7936c1e5e0a8dba91dbb", file: "parakeet-tdt-0.6b-v2-Q4_K_M.gguf", bytes: 475491840, sha256: "4853f9653f641d376e6f7de65d73c7a34a73677704a606727bf51acc83f999f3" },
    Entry { family: "Parakeet", repo: "parakeet-tdt-1.1b-gguf", revision: "bc3855fb729e7d925e1e19c99f7b4f0fe195fb44", file: "parakeet-tdt-1.1b-Q4_K_M.gguf", bytes: 825248416, sha256: "38e85773763afe7b79c8538fbd091367b4644123358ea209eeb665879e3fcb13" },
    Entry { family: "Parakeet", repo: "parakeet-tdt_ctc-1.1b-gguf", revision: "3c3c2d921217597c4b3027e4012b524bad9c8bdc", file: "parakeet-tdt_ctc-1.1b-Q4_K_M.gguf", bytes: 825248000, sha256: "5c27d9289e23019c459b1cef87baa5b672343733ccd3762796e68aec447169cd" },
    Entry { family: "Parakeet", repo: "parakeet-tdt_ctc-110m-gguf", revision: "1c3f0d5378fdd358905cc410a93ed9e64f52d170", file: "parakeet-tdt_ctc-110m-Q4_K_M.gguf", bytes: 89989600, sha256: "486414fd90185a8c8a4ced7c123cfb133ff4f7958426c6b8bd9049946b56b448" },
    Entry { family: "Parakeet", repo: "parakeet-unified-en-0.6b-gguf", revision: "843b1b175ace04f5a89d379c79e2518c0d065b25", file: "parakeet-unified-en-0.6b-Q4_K_M.gguf", bytes: 477274496, sha256: "a8bf3de2b393bd14ead5a858c3748d5e3b07a20fdeabdd3b498fba4f463fa929" },
    Entry { family: "Qwen3-ASR", repo: "Qwen3-ASR-0.6B-gguf", revision: "7e7c69a577fa2725dfedbe34b5e999336e30a19b", file: "Qwen3-ASR-0.6B-Q4_K_M.gguf", bytes: 589560480, sha256: "5b58f32a58ffa2c8783e0b0963485623e286e6272d953dfc9e28bc3447dee0c0" },
    Entry { family: "Qwen3-ASR", repo: "Qwen3-ASR-1.7B-gguf", revision: "0e0a596e3959c3f3f2ab7d443a5b6f0f4a4bba6f", file: "Qwen3-ASR-1.7B-Q4_K_M.gguf", bytes: 1319830496, sha256: "b7afe3674f653fa84f712ed2440353c6e7cf7f93697fef76b05a26538b24844e" },
    Entry { family: "SenseVoice", repo: "SenseVoiceSmall-gguf", revision: "ff8f085a8fa2e5ccae0e849ef19ebc6dc18e608b", file: "SenseVoiceSmall-Q4_K_M.gguf", bytes: 145738304, sha256: "9d775303e6b4e793d049ef226214c400b2b1746c734ed017146a1a085b13d02b" },
    Entry { family: "Voxtral", repo: "Voxtral-Mini-3B-2507-gguf", revision: "3d4e8e74a2ada47b400b2957575b2a9c058efe80", file: "Voxtral-Mini-3B-2507-Q4_K_M.gguf", bytes: 2984721056, sha256: "3a6717aa8f8989108d260cbd237584289eec43cc987e10133c33643515936205" },
    Entry { family: "Voxtral", repo: "Voxtral-Mini-4B-Realtime-2602-gguf", revision: "7fd3d424fc558edca2c53808578b0541ef52a81a", file: "Voxtral-Mini-4B-Realtime-2602-Q4_K_M.gguf", bytes: 2830493984, sha256: "39dc1f65539373a406edea7490505822d77c12edff521744678717eef4da4723" },
    Entry { family: "Voxtral", repo: "Voxtral-Small-24B-2507-gguf", revision: "a90f97f24e3d4252647d79f6046ef19d04ad1c5d", file: "Voxtral-Small-24B-2507-Q4_K_M.gguf", bytes: 14302261728, sha256: "0da1b866c0eb2ce805678c18a7fb1e413320e4ad3ccbef7160707b65bae937a3" },
    Entry { family: "Whisper", repo: "whisper-tiny-gguf", revision: "1bd61cda36bbd22c88fc5a33842498a6e1d30f45", file: "whisper-tiny-Q4_K_M.gguf", bytes: 43621792, sha256: "86e1100470df173fa9079f941e33e4d52d11c03923407dd1ec51fd1937d8e609" },
    Entry { family: "Whisper", repo: "whisper-tiny.en-gguf", revision: "9dd46e00ef0577e354e658c07b3ac5580e9cd211", file: "whisper-tiny.en-Q4_K_M.gguf", bytes: 43545248, sha256: "3bfa6200aa12a21409445401f7871b5c733546dc45a29eb4871fcb3c7954e08b" },
    Entry { family: "Whisper", repo: "whisper-base-gguf", revision: "42fcd2077de47c36d779309e0e91b76ec620f56d", file: "whisper-base-Q4_K_M.gguf", bytes: 58870848, sha256: "78bc9a8cb7e16a3354b262a4c71c0ed4892f6a66b681068f474dfada0729c0ae" },
    Entry { family: "Whisper", repo: "whisper-base.en-gguf", revision: "0dd374130837de419035c77d76440aad61ca135c", file: "whisper-base.en-Q4_K_M.gguf", bytes: 58794272, sha256: "fd19959ff81f96c2b66f26fad97aada090ed6b3af681cf5b569bef3d7a073da5" },
    Entry { family: "Whisper", repo: "whisper-small-gguf", revision: "4afdc7942ecf1997f359deae48a63506b062eac4", file: "whisper-small-Q4_K_M.gguf", bytes: 171630656, sha256: "b204d2005a3e5d4fe6153bd61e5e8b32e757ff7b017ac8f61c6f051c2f80e939" },
    Entry { family: "Whisper", repo: "whisper-small.en-gguf", revision: "ab86448a84fcc15a96a9a2216d93a8dac8427da3", file: "whisper-small.en-Q4_K_M.gguf", bytes: 171553856, sha256: "7bf9ebce1e6332b3d18d4c27032e3097148fc124befed3e70022e0d70539a365" },
    Entry { family: "Whisper", repo: "whisper-medium-gguf", revision: "60cfb00405a9a5bb85ee347ef93b68df7a311dc5", file: "whisper-medium-Q4_K_M.gguf", bytes: 504102848, sha256: "6c9cad9c41d1fa0fcc624898f7ee3485b66127627878081a5655c6f6f3ce20ab" },
    Entry { family: "Whisper", repo: "whisper-medium.en-gguf", revision: "e6370f41cfa6d73e2a89969f4f5055f8c7e4dbb6", file: "whisper-medium.en-Q4_K_M.gguf", bytes: 504025856, sha256: "8a0eb0bb20019af88fc0ccda06d97dc949a6a0cdf35d3f2c316b967e76cd4077" },
    Entry { family: "Whisper", repo: "whisper-large-gguf", revision: "3fd31d8c72ee5fd06ecc6b4a74f85a951c106173", file: "whisper-large-Q4_K_M.gguf", bytes: 996526048, sha256: "7d7693afc06096de46f2a41309ac479909e131824e78dae4e205f35463aaec09" },
    Entry { family: "Whisper", repo: "whisper-large-v2-gguf", revision: "86dd8fbcd7324624f86d826e7fc241f4922ee987", file: "whisper-large-v2-Q4_K_M.gguf", bytes: 996526080, sha256: "76aa37b205abc1fb7a9e7aaf0655b8747995b81e6bb72c18f4b1acf59e222f79" },
    Entry { family: "Whisper", repo: "whisper-large-v3-gguf", revision: "274ad1d1cf680b7ee53cc70f78461394cd3a344b", file: "whisper-large-v3-Q4_K_M.gguf", bytes: 997303008, sha256: "6fe933811cec4cd3159debc46520ecd3aac6c7e322ece2ac61fcfdab184e1fe0" },
    Entry { family: "Whisper", repo: "whisper-large-v3-turbo-gguf", revision: "ceea6c8a94a21ab85be244d311e874a39344dbf5", file: "whisper-large-v3-turbo-Q4_K_M.gguf", bytes: 536069728, sha256: "ecfe9b6beb4ab18fef49187cc968cc74b5168b94629c8830e2ca6b794c6e25ed" },
];

impl Entry {
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/handy-computer/{}/resolve/{}/{}?download=true",
            self.repo, self.revision, self.file
        )
    }
    /// Short id: the file name without `.gguf`, lower-cased.
    pub fn id(&self) -> String {
        self.file.trim_end_matches(".gguf").to_ascii_lowercase()
    }
}

/// Finds a model by id, file name or unique substring.
pub fn find(query: &str) -> Result<&'static Entry, String> {
    let q = query.trim_end_matches(".gguf").to_ascii_lowercase();
    if let Some(e) = MODELS.iter().find(|e| e.id() == q) {
        return Ok(e);
    }
    let hits: Vec<&Entry> = MODELS.iter().filter(|e| e.id().contains(&q)).collect();
    match hits.len() {
        1 => Ok(hits[0]),
        0 => Err(format!("no model matches '{query}' (see `ostt models`)")),
        _ => Err(format!(
            "'{query}' is ambiguous: {}",
            hits.iter().map(|e| e.id()).collect::<Vec<_>>().join(", ")
        )),
    }
}
