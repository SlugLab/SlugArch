# Model Provenance

## Upstream
- Upstream model ID: `dealignai/Gemma-4-31B-JANG_4M-CRACK`
- Upstream license identifier: `gemma` (as tagged on Hugging Face at artifact preparation time)
- Upstream revision/commit: not pinned in current deployment contract (recommend pinning before public replication)

## Transformations
- Distillation/quantization pipeline: Gemma-based teacher distillation to a smaller custom INT4/KAN deployment model (`weights_int4_FINAL.bin`)
- Weight format: INT4/KAN
- Tokenizer source: `/home/sovryn/tokenizers/dealignai_gemma/tokenizer.json`

## Runtime Contract
- model_contract.json SHA256: `872f880f8103ec1549f9de87988f19307075a26b1b99be59d1bac275935d077e`
- tokenizer.json SHA256: `3151898c022536cf420b732dd2fcbf8e7c456cd39711a27f9b82a7ced72b6c83`
- weights_int4_FINAL.bin SHA256: `7ba5c0c5b350a8b0c50c7ec7fe30b64064bee4f13ce6d588eeb826d84d3644ce`
- omni_titan_agi_top.bit SHA256: `983ab226ae213f984dc0eb33f427dc51a486b79a111d6d3e719344c660b1070b`

## Notes
- This release contains deployment artifacts only.
- Re-training and re-synthesis are out of scope.
- Teacher line and deployed model are not equivalent in size/residency; deployment uses a compact distilled artifact path.
