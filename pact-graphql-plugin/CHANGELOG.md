# Changelog

## [0.2.0](https://github.com/mefellows/pact-graphql-plugin/compare/v0.1.0...v0.2.0) (2026-09-08)


### Features

* add exact/semantic/subset query matching modes ([ae6f75c](https://github.com/mefellows/pact-graphql-plugin/commit/ae6f75c10867259c49bb8df66302cd70075c4d37))
* add GraphQL interaction builder ([09114cf](https://github.com/mefellows/pact-graphql-plugin/commit/09114cf33d157e917956c2fb42b4de6c47cf1fae))
* add GraphQL request encoder ([b4c61f0](https://github.com/mefellows/pact-graphql-plugin/commit/b4c61f031dddf983d00d2383756d46e65a1eadbd))
* add schema registry ([35f72ff](https://github.com/mefellows/pact-graphql-plugin/commit/35f72ff2944974ade1c69e2aa07cc2485ae435d4))
* canonicalise query documents through the AST ([f598ef3](https://github.com/mefellows/pact-graphql-plugin/commit/f598ef35ff9089e6fa330dbe35eee759997d7f2a))
* carry enum members and field return types in the schema index ([12fcf4c](https://github.com/mefellows/pact-graphql-plugin/commit/12fcf4c869ff2de006636226859ab97ad4f6781e))
* derive matching rules from the GraphQL schema ([c1fb255](https://github.com/mefellows/pact-graphql-plugin/commit/c1fb2552422f6378d197d1408837e3721153cd52))
* enrich graphql interaction config ([bd15af3](https://github.com/mefellows/pact-graphql-plugin/commit/bd15af3b1691af1472f696a4c116d3c11343ff5e))
* implement GraphQL pact plugin server ([2ffb5b1](https://github.com/mefellows/pact-graphql-plugin/commit/2ffb5b1e0f6bc9b96deffa65eb2583f0271614b3))
* matching contents and revert debug statements ([d0d4bc3](https://github.com/mefellows/pact-graphql-plugin/commit/d0d4bc39bfc5db6a8449c8c806eac6689305a4a3))
* report query mismatches at the field level ([eae0e34](https://github.com/mefellows/pact-graphql-plugin/commit/eae0e34a39b8bb550c0de1d7322243230f56ff14))
* validate GraphQL responses against schema and selection set ([253d297](https://github.com/mefellows/pact-graphql-plugin/commit/253d2972ecc25ee98739434ffc6f5a4b78b2ea0f))
* wire GraphQL response matching into the plugin protocol ([22ae2af](https://github.com/mefellows/pact-graphql-plugin/commit/22ae2afc0cb5e6347403a861a0ea91c5ef6c60b9))


### Bug Fixes

* align pact plugin driver dependency ([b4c3eb0](https://github.com/mefellows/pact-graphql-plugin/commit/b4c3eb02239d4d65f0c8c9c199c4d0d1673c374d))
* **core:** correct pact compatibility, validation and startup ([996b78a](https://github.com/mefellows/pact-graphql-plugin/commit/996b78a814a31918d1e45c47a817a6540aaaeafd))
* default transport during deserialization ([d3227ed](https://github.com/mefellows/pact-graphql-plugin/commit/d3227ed0658c920ac166e452173c3b1aa014c7eb))
* normalize GraphQL query string variables ([2f1c10e](https://github.com/mefellows/pact-graphql-plugin/commit/2f1c10e1dc5b08e20780446425810cde036a365f))
* preserve GraphQL JSON field order ([f3915bc](https://github.com/mefellows/pact-graphql-plugin/commit/f3915bcaa19ac763f7bfcc2ed3ecda63872ddabc))
* propagate real canonicalisation errors and detect fragment cycles ([0e4782b](https://github.com/mefellows/pact-graphql-plugin/commit/0e4782be8754de822b1e1daf65aa4246d77826e5))
* return a single interaction part per configure call ([22537a5](https://github.com/mefellows/pact-graphql-plugin/commit/22537a5fd21f9202e2831ec931a07ecfd474a457))
* **schema-index:** revert pub(crate) widening, move tests in-crate ([b0506c8](https://github.com/mefellows/pact-graphql-plugin/commit/b0506c8e2f1532c91066fcfbb98bcb4a878f64c2))
* stabilise graphql example ([252214f](https://github.com/mefellows/pact-graphql-plugin/commit/252214facc2e3b8704b1cf7ddeab67e14d767173))
* surface graphql validation mismatches ([2607fc1](https://github.com/mefellows/pact-graphql-plugin/commit/2607fc1494a9ca7af6dd837bd37467052a3d04fc))
* validate schema ref hashes ([6c2f920](https://github.com/mefellows/pact-graphql-plugin/commit/6c2f9203d64ad7a221a0271f50422e54a35f5980))


### Documentation

* explain the per-part plugin configuration model ([cb48c1f](https://github.com/mefellows/pact-graphql-plugin/commit/cb48c1f362cf86c980a6f2d762bc9a6c4528849f))
