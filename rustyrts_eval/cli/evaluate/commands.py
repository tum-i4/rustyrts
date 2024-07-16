import os

import click

from ..db.commands import SUCCESS_CONNECTED_MSG, FAILED_CONNECTED_MSG
from ...db.base import DBConnection
from ...evaluation.walkers import mutants_walk, history_walk
from ...util.logging.cli import (
    click_echo_success,
    click_echo_failure,
    start_spinner,
)
from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)

# mutants
SUCCESS_MUTANTS_MSG = "Completed mutants evaluation successfully"
FAILED_MUTANTS_MSG = "Failed mutants evaluation"

# history
SUCCESS_HISTORY_MSG = "Completed git walk successfully"
FAILED_HISTORY_MSG = "Failed git walk"


########################################################################################################################
# Projects configuration for mutants evaluation

MUTANTS_PROJECTS = [
    (
        "projects/mutants/tracing",
        "master",
        [("3de7f8c6016aebc22228375dc9100c02e955c6d4", (None, None), (None, None))],
    ),
    (
        "projects/mutants/tabled",
        "master",
        [("cc4a110d5963b7eede0e634c83c44d9e8b8250e4", (None, None), (None, None))],
    ),
    (
        "projects/mutants/regex",
        "master",
        [("b5ef0ec281220d9047fed199ed48c29af9749570", (None, None), (None, None))],
    ),
    (
        "projects/mutants/rustls",
        "main",
        [("45197b807cf0699c842fcb85eb8eca555c74cc04", (None, None), (None, None))],
    ),
    (
        "projects/mutants/rust-openssl",
        "master",
        [("cc2850fff7c4b0d50a23e09059b0040044dd9616", (None, None), (None, None))],
    ),
    (
        "projects/mutants/pulldown-cmark",
        "master",
        [("967dd38554399573279855a9e124dc598a0e3200", (None, None), (None, None))],
    ),
    (
        "projects/mutants/orion",
        "master",
        [("cfa2c0c1e89f1ec3d2ab1ab1d57f88c1201e452c", (None, None), (None, None))],
    ),
    (
        "projects/mutants/ripgrep",
        "master",
        [("af6b6c543b224d348a8876f0c06245d9ea7929c5", (None, None), (None, None))],
    ),
    (
        "projects/mutants/rbatis",
        "master",
        [("0149c2862842771dd5a22a7ef69c9501053f546a", (None, None), (None, None))],
    ),
]

########################################################################################################################
# Projects configuration for testing evaluation


TESTING_PROJECTS = [
    (
        "projects/git_walk/exonum",
        "master",
        [
            ("cb1a0384651bd31a869ab1aa65e1806a53a3d7bb", (None, None), (None, None)),
            ("78fe27b23209beeb8f55ad2308835de730f03f38", (None, None), (None, None)),
            ("3bc48f285343b379738465ff7ae05efb5730b908", (None, None), (None, None)),
            ("a295f8744fa4d7511f0058766ef7cbd670cb1d84", (None, None), (None, None)),
            ("c36483f7af6c2e1626e48cb83f285310d7c2fa59", (None, None), (None, None)),
            ("262ece75ac53dd39cce23d427c03089f30a4c60b", (None, None), (None, None)),
            ("8f25d7822fd232c691747ae512f13da92f100ea3", (None, None), (None, None)),
            ("d01f37122b6dbca31e37b3422777f2f442160ed9", (None, None), (None, None)),
            ("53898fc17b2c1b20e4761d45194d54560c25633d", (None, None), (None, None)),
            ("28e8d8393005b2a9cda728168dc1963ab7c7ff4f", (None, None), (None, None)),
            ("65b576c311a3140a2f54f54d0853b54d282618ff", (None, None), (None, None)),
            ("f5a785aa5349ee30375ff51940a930c2e1cf4ed0", (None, None), (None, None)),
            ("7d73d1516935d7710dfec13d1e2ea1884a38ebb8", (None, None), (None, None)),
            ("59d7557dc36479bb60edadd6159baa9327b0b2f3", (None, None), (None, None)),
            ("3c93db7e6feceadb17f3f143c0ed856db2958ac0", (None, None), (None, None)),
            ("882e23d65c916429c19e85b91ee9b5826908a71d", (None, None), (None, None)),
            ("29f596b404a163e2f2414b6e19360600e89ce506", (None, None), (None, None)),
            ("b8e4797aab905cd389bb46d4d7238d721d80cdb8", (None, None), (None, None)),
            ("b614e41feecd3b6d775c492a3e56f49155b908ea", (None, None), (None, None)),
            ("87280613d8a66e74f1a7b00ba09f14cccb7e8dd5", (None, None), (None, None)),
            ("e75b31a73cf9f09139c77c920c3638eeecdcba2c", (None, None), (None, None)),
            ("bdbfb4abdf63dbc0c5fd9f627dccde5c4af5ffef", (None, None), (None, None)),
            ("05b435e5d930d8d05ad01839a6241fb62dc3950d", (None, None), (None, None)),
            ("47d43c73af945923cd22dd8b989a64d5178a86bd", (None, None), (None, None)),
            ("b7618adeecefdc774d0581b5fca180b1e4d7f7dc", (None, None), (None, None)),
            ("334a37e39369a2aa3d2922f2d03d6f0c24d175d2", (None, None), (None, None)),
            ("38710ad2f37a8d1f8067c1b6186604a33131f742", (None, None), (None, None)),
            ("517b82efcd2788531183858cca8d4abb3edbadf3", (None, None), (None, None)),
            ("57109131297cb926cb652910c1e0228effc16b58", (None, None), (None, None)),
            ("fbc3d17a0993a7cfe8dd7addb4609a551eb53148", (None, None), (None, None)),
        ],
    ),
    (
        "projects/git_walk/feroxbuster",
        "main",
        [
            ("216e0e6595fbf9329656983b047f2b2b7d78977d", (None, None), (None, None)),
            ("f6047e98197a77e70ce4f599bfaffd35b9e0c67c", (None, None), (None, None)),
            ("36994d208df230b0238342ba56005dd7c3d8598d", (None, None), (None, None)),
            ("3de31f0393bd46944031560e9c772e20c12714ea", (None, None), (None, None)),
            ("bb4a335299e91972a644612d81b37eb50b25938a", (None, None), (None, None)),
            ("c23850208b9e4f0d74af77f7762856fbd2fd5443", (None, None), (None, None)),
            ("7f145f11df685c2a177947d91bdb0ca62b639b33", (None, None), (None, None)),
            ("53281c0921a57d7adadb00596bf0828ccb07a3e4", (None, None), (None, None)),
            ("65967591322d3abfe5f135d1dcb0097d074f87ce", (None, None), (None, None)),
            ("acf16c92cd7756aaf9afa066344018caf42568cd", (None, None), (None, None)),
            ("329d04252f9bcb302bdd5508cd14ee72df75b909", (None, None), (None, None)),
            ("5578e8db5c16b3d07b05bbeedf54a2b917b806ec", (None, None), (None, None)),
            ("fddade6a1173795e3cb60987d33670e99e3e5f71", (None, None), (None, None)),
            ("43e5ad14c9a543e920659a218d92587a71111f01", (None, None), (None, None)),
            ("f173147352978d3e7a0ab65ac620d7a480d1d593", (None, None), (None, None)),
            ("d36379ba1bafff25e12bff2c6293f83bb6bc50ff", (None, None), (None, None)),
            ("fe5612ce71558b826a9f1f6535de309dea117db2", (None, None), (None, None)),
            ("6c3e41fc3d7dd676e472c343b4f17208e2e92204", (None, None), (None, None)),
            ("3f5ff1ad3e4847f5bd5e525c82db511444a37b50", (None, None), (None, None)),
            ("28ecd60f61c52c2960b4505b8e05a5a7590bc96a", (None, None), (None, None)),
            ("eddab0de13014e97cd68f708b7f5c6eb6f452d37", (None, None), (None, None)),
            ("4f557511b4974ee51ccdaee788907ac1edbc9674", (None, None), (None, None)),
            ("c99f6146e34db48163a584658892892398473662", (None, None), (None, None)),
            ("4debe68ed67539d146101cd9c4acb3e85d49c42f", (None, None), (None, None)),
            ("3a128df2fc416e7b3a7f3dc827e4dfa7dfc6ef0a", (None, None), (None, None)),
            ("f3b2193b2f536bd0674ba2d085f39a3ffa5631a2", (None, None), (None, None)),
            ("9b1a24bca3e0e1ec1d1310b0881ccb852de4b0df", (None, None), (None, None)),
            ("b456215a00816fb4372c1a260b76ed0fca175a0b", (None, None), (None, None)),
            ("7f0dcb6b46ac00c27dbff4913548f2d70b7ba8a0", (None, None), (None, None)),
            ("82f8f687fd18f46b21e81f31ea189dc9fe29a1cc", (None, None), (None, None)),
        ],
    ),
    (
        "projects/git_walk/meilisearch",
        "main",
        [
            ("b99ef3d33644b24cccf9ff167f698b30b99625d1", (None, None), (None, None)),
            ("953b2ec4380736136c015ccbf4d508acdf7fc21a", (None, None), (None, None)),
            ("79fc3bb84e3659797fc3ca962854fc026e7b6684", (None, None), (None, None)),
            ("2b944ecd893c3421baf529ffa060aecb74e5a88f", (None, None), (None, None)),
            ("44f231d41ea1ebd349240413d4d38a05ea961d79", (None, None), (None, None)),
            ("c09e610bb5e921ff03b2b469c6691ba9be2d231f", (None, None), (None, None)),
            ("2ef58ccce9a5016f8563f4bfd8c73c5dda322582", (None, None), (None, None)),
            ("0990e95830a43dd7b0b4f513f86f442efedc42d5", (None, None), (None, None)),
            ("6af769af20039d258126f2983b36d0145a2cb38d", (None, None), (None, None)),
            ("80f7d873563e7f06acde532d4345f4a866390a14", (None, None), (None, None)),
            ("8dd79426565660e211431e43ff38e667b502092c", (None, None), (None, None)),
            ("f892d122de2f6d41c7228cc25be270fa0b689041", (None, None), (None, None)),
            ("ac2af4354d9d2c55d87199e7ea9928d4c1d8c574", (None, None), (None, None)),
            ("65ca80bdde170fba76a2d3175b1580ec0654c3cb", (None, None), (None, None)),
            ("da04edff8c6b032d70fe58500c530d8b44cd04e1", (None, None), (None, None)),
            ("2f3a439566a3257c6722d6b3eca367e84e559f1e", (None, None), (None, None)),
            ("cb71b714d71b025f0b08b9408191c22cb6700798", (None, None), (None, None)),
            ("9b660e1058eabe31ff1ebde64043f6970afee851", (None, None), (None, None)),
            ("d263f762bf88d7e2209d0d849578fd7463606eb4", (None, None), (None, None)),
            ("eab1156f8cee2d417ca5242578bc10ca387938ad", (None, None), (None, None)),
            ("2830853665a20f751389fb109802bbb1609348fc", (None, None), (None, None)),
            ("348d1123882b1c3433d43b179a30d1cf1457fda9", (None, None), (None, None)),
            ("5ac129bfa161a0aceda88382ace169c115c43f32", (None, None), (None, None)),
            ("b3f60ee8057f837a5de9107db42faa1cfd4fde17", (None, None), (None, None)),
            ("d1b364292322028d4ea4fea820b70c13a3bac222", (None, None), (None, None)),
            ("6a742ee62c250a0108ef6706c9c52b462b3a2c8f", (None, None), (None, None)),
            ("11f47249578871c7f61f3b2fa9c2184c635d2f77", (None, None), (None, None)),
            ("05ffe24d645d3b6dce270b94800a2b17ab024fbf", (None, None), (None, None)),
            ("cb1d184904e474d178d0041bfe6b9ded0638b0b6", (None, None), (None, None)),
            ("c9246145274634f3df1b17bd578dae7eb6f94ddd", (None, None), (None, None)),
        ],
    ),
    (
        "projects/git_walk/rayon",
        "master",
        [
            ("f0c5a47fca1f2a1ba627aa9ed0514eb0855147c4", (None, None), (None, None)),
            ("b98bb23f0595cbf56afba8f641b366a0433f002e", (None, None), (None, None)),
            ("16b3310e30e4d8af11ccd0d76b3e319816b892d3", (None, None), (None, None)),
            ("2d62dc6b89895fb28210acf0e160669654b970d1", (None, None), (None, None)),
            ("e20b4ede19accb0d65494016fa5bfcd46b636e5d", (None, None), (None, None)),
            ("b3e6600ca34054d8cefa61fbee4245d1dccd6ba5", (None, None), (None, None)),
            ("ed6a5f75c4dc1cb0ac4f59f6f01b38f8a695a777", (None, None), (None, None)),
            ("545feac3b8bbe7c0da42cb8a3aa60c17a9b16829", (None, None), (None, None)),
            ("edaf85171b8acbe968586b676a309cd46225c472", (None, None), (None, None)),
            ("e7ed9a857e23e8b216c97adc54a52830183df40c", (None, None), (None, None)),
            ("a7f5c779761b34293bcc4c16e420c4c5fc4a1c57", (None, None), (None, None)),
            ("35c032c6714d6fd9876976c5ad6793f8052d7ff0", (None, None), (None, None)),
            ("aa6fdd3f1b1b2411f546df16fff9f805e2bfa93a", (None, None), (None, None)),
            ("33790dc7eb68a114feacf00547bd60a3c5ad3c3d", (None, None), (None, None)),
            ("d2bef7e2d8524011f01b06d79711a11b99b9316e", (None, None), (None, None)),
            ("b56c6b8fe89b474cf07380730ea4bb660406a3a6", (None, None), (None, None)),
            ("d31d3e3b8ee85db761f283cca25de43a7b7931be", (None, None), (None, None)),
            ("d466fd04a75325f7e6a37b1f142181dbb1e20c12", (None, None), (None, None)),
            ("4fd13b033424be5eac826571e017b1a008d0bd06", (None, None), (None, None)),
            ("a0efb4abb5bc3a2ee0ed812392a2b7386dc2dd38", (None, None), (None, None)),
            ("f7d75532fcbc8151b97dec85131e5a0be3db4b4f", (None, None), (None, None)),
            ("54758597379d34108d55343cdb4a5f390f7e2ec4", (None, None), (None, None)),
            ("faf347a416e8998ace15e795d0af9040cd0ec15e", (None, None), (None, None)),
            ("51f9676ac4a743d124ed88878d611a8b475f488f", (None, None), (None, None)),
            ("dd91c279709dfbaead9f70b28854ce632d040a0f", (None, None), (None, None)),
            ("d41d29ab56d08c9c7836fcacf415e457488b574f", (None, None), (None, None)),
            ("a116575410ce3e2e003f10c64c565e85ac5b3893", (None, None), (None, None)),
            ("c396338d8c3ebec81dff95965d89673b8cdfab9b", (None, None), (None, None)),
            ("08345e14b0cca55fcc9a427ef6ef057e65ab6dee", (None, None), (None, None)),
            ("073b79b22f23dc78769521555388d18897b48b26", (None, None), (None, None)),
        ],
    ),
    (
        "projects/git_walk/wasmer",
        "master",
        # Only one feature out of {test-native, test-jit} is feasible (otherwise compiler error) Sometimes only one
        # feature out of {test-singlepass, test-cranelift, test-llvm} is feasible (otherwise compiler error)
        #
        # Feature test-llvm has lead to a compilation error and has therefore not been considered
        # Feature test-native has sometimes lead to a segfault and has therefore not been considered
        #
        # We selected to use test-singlepass and test-jit if applicable
        #
        # We selected test-no-traps or coverage if present,
        #   to prevent using signals that might mess witht the traces of dynamic rustyrts
        #   or lead to discontinuations in the graph of static rustyrts
        [
            (
                "d2caa60fa1dbc134846a52af945b9661dd498577",
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
            ),
            (
                "6f2957b4b364c0c664d66e40588282ad5e95a785",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "44339c18fe782c19ad25a2ae63bd11e0cb4fe011",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "c82fd9377d3fc90bb2876e346d407fffe29a2cfc",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "c91e545e03d1a24b083d3ea7368ea0d160e45a4b",
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
            ),
            (
                "94374e4e98e402052c60a97a983336183c811146",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "499eb499f01f9870bb1bbd8e768d6a6b6a020a2f",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "8074253fe24c62a3e6bbd9ea8e808409eaf9bf24",
                ("--features test-singlepass,test-universal,coverage", None),
                ("--features test-singlepass,test-universal,coverage", None),
            ),
            (
                "9353d09cfabd8a297f3184ec9a5a3787b539c607",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "36419ec7e48a9efecac336bd9cf4b120bc5e13f1",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "03b34144bff602c549dcfbee52520d33252d0e04",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "bb6f483fac97dbad4a85286970c78809cba4abc3",
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
            ),
            (
                "a91d2474619388f72b682196ccf70d75289e1745",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "54b4495b3f9a9e3dd60cc1bf00d20d07eb777bab",
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
            ),
            (
                "df30d25d3952ce4b04a75c4353fe8f90df987b1e",
                ("--features test-singlepass,test-universal,coverage", None),
                ("--features test-singlepass,test-universal,coverage", None),
            ),
            (
                "66bfb88d8374be4ffdbfae3257bfc30c6bde0c1f",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "9e83e8a4663e4f5f8e95a8d823e96b8a6390ec50",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "19c4c623a819f3f3bac28af91d749254fa82fcfc",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "f8c0910c337a0e571abcf457f42e936aa98e275e",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "9ecedc3925f7c764916025c78bdc626cf45ef46b",
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-dylib,test-universal,coverage", None),
            ),
            (
                "c03d61b78a1a0bbd5f542cf1235e88b6a639ee6a",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "59d065ee32a24e8f82e6c762c4d6ecaa3a481879",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "88b4093646f60e680dc3fa3be9877c2ad77257a0",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "a869daea916863cb63e6fcebe955417aebd3b5b4",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "156f0483b3c7868bcf3fb56f84b1c35cd8eb9531",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "9ae02d5c0b66bcaff799ad4c64ab70057bc01e09",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "1dad3dd1cb6b94b0197938134839cf8fe9fbe9a5",
                ("--features test-singlepass,test-universal,coverage", None),
                ("--features test-singlepass,test-universal,coverage", None),
            ),
            (
                "3d6241236c50bf4f8f378c50abeb84b733116a00",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
            (
                "1dd6d8d3eb4f2633e1f05acce50803302f8a3498",
                ("--features test-singlepass,test-jit,coverage", None),
                ("--features test-singlepass,test-jit,coverage", None),
            ),
            (
                "59b37e4b75a5b0d5cd701858defbfbc99dbace13",
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
                ("--features test-singlepass,test-cranelift,test-universal,coverage", None),
            ),
        ],
    ),
    (
        "projects/git_walk/zenoh",
        "master",
        [
            ("431ffd48f6dd4192f63a168efbd84bcfa7142749", (None, None), (None, None)),
            ("472cc34acb9d85f781a1c6fe735fa124c88ce97b", (None, None), (None, None)),
            ("0e791bc11d6819f68d143072f7bf1bb60b341ebf", (None, None), (None, None)),
            ("f7ff3f4278b96ef60eb14f8907c36d5cc2795dee", (None, None), (None, None)),
            ("7ec9427f8cb091387802d6a7e1dd910248941ef8", (None, None), (None, None)),
            ("974441cd7addd8bb22ed7a819652fee9115b9d1e", (None, None), (None, None)),
            ("85409a54adb7598bf5d07b95ccc4d01a2f0331e3", (None, None), (None, None)),
            ("e144f3d502ac69f9ab2a9b612525a7f67b3f2a1e", (None, None), (None, None)),
            ("eab33b2a9f6a136f5a2b9823358d58690741b0be", (None, None), (None, None)),
            ("26c77ce77dee5f850358df3a696c45690cdfe5e9", (None, None), (None, None)),
            ("8b78e6a090cad12a8f0271667025085538cfb389", (None, None), (None, None)),
            ("3d94ac1a22268fa06ee0c76cdd385ba1ce454a4a", (None, None), (None, None)),
            ("3302c23929e4e66f5aced4df5e03b4eafd8b2f3f", (None, None), (None, None)),
            ("2a806883239b1a377e5012b610973562c1127f6e", (None, None), (None, None)),
            ("65fed69e8500e066375568f69d1616779a83e509", (None, None), (None, None)),
            ("38f7334a9e8fac86f631b254d5e64a5053e5b671", (None, None), (None, None)),
            ("0cb17cd9017eb9555819ad9c3d65ccfb10feee0a", (None, None), (None, None)),
            ("059c5bf3cfca916a3dbaa7d4ca752edff9a3205d", (None, None), (None, None)),
            ("59d6b90306c377982e45c2941b935a5efddf436a", (None, None), (None, None)),
            ("1617651e0a961bfa406f3deb58647906b34c02fc", (None, None), (None, None)),
            ("91eba62fe07fbabfe2a0421ba57caa20e7e88db6", (None, None), (None, None)),
            ("9405b41e8756c3652837b3f40d3075c6b7ee9bdc", (None, None), (None, None)),
            ("bd237c2e3efbe690f61df1126b7e016a216f6cbd", (None, None), (None, None)),
            ("ba371c8a76755c64379ed144ec55311775141d1e", (None, None), (None, None)),
            ("14dcff357dd57410dab339c9b0db9a3d5dff8c0d", (None, None), (None, None)),
            ("a8b6de0d27b384ee0f48154b2a3b93c0acc2b7e6", (None, None), (None, None)),
            ("89b88cbe40e4d40b8c2351cd903ef127395ceae8", (None, None), (None, None)),
            ("5bd713eb8ed1ea4d68010c1c4f7d2dcd2a95922b", (None, None), (None, None)),
            ("3fcace64b931531fd44385d9b491909586033063", (None, None), (None, None)),
            ("6f98a5ee336f98cb592838451c382b154499d0c4", (None, None), (None, None)),
        ],
    ),
    (
        "projects/git_walk/penumbra",
        "main",
        [
            ("4235275e5033945ad7ad4a2137326cf4d75380ed", (None, None), (None, None)),
            ("8cd239964ef0a45c41b36fa2c16e4dd9412eeffd", (None, None), (None, None)),
            ("c6b53389f5e0968aa08b1c849f95d0438c6a8f70", (None, None), (None, None)),
            ("9f04762adbd96dbd04c7a36e84cd74568e81f915", (None, None), (None, None)),
            ("fc2edd093b94dddc9fbf42a7b409343942681138", (None, None), (None, None)),
            ("c2bc2c0fc498dbd7e1134ffea18cc3452286685c", (None, None), (None, None)),
            ("4574c1acc26b8f83f264ec46be1ee48fb4ab4010", (None, None), (None, None)),
            ("cc784a2143b6ce04c5a164b102aeb5e819542e73", (None, None), (None, None)),
            ("ac48951257a3608584fbd6647ea2e26effde67c4", (None, None), (None, None)),
            ("b0f80fa28bb69af2a1b0433930b1697fb0666d68", (None, None), (None, None)),
            ("89c5b3d42c218ffd8f04aed8e707ca9c8c0edf0f", (None, None), (None, None)),
            ("0dfff7b47d840e8d4f66dc1998f87dc9d4ed8670", (None, None), (None, None)),
            ("6f504cec09e6900f2818f4c2bf2e5deb249eae7d", (None, None), (None, None)),
            ("210d2b9c1cd0d9cd99ec20eeff0114652ac178c1", (None, None), (None, None)),
            ("ac6fcef3420905eaed7020fc8226d71a3df8562a", (None, None), (None, None)),
            ("46e6a611caf97e11df54e148acde84f23613d6cd", (None, None), (None, None)),
            ("66c3f91f43462100c9a83dce85382fd09fee6f26", (None, None), (None, None)),
            ("954cdbf58d4bf23f589b4c0edb5d938713ced25c", (None, None), (None, None)),
            ("ca7f0ff3b7235d5b8a4334036eabae3cd1705b09", (None, None), (None, None)),
            ("b9d325098a1acd260c5efaa094e74e0271161f56", (None, None), (None, None)),
            ("686ff689e05c2cabf607760a04fc1d82a8fb584f", (None, None), (None, None)),
            ("96189b4567e89fee841d6e327b3a49a5f9870891", (None, None), (None, None)),
            ("86f2b9839ee2d1ed4dbb2c6d7217687df6f73125", (None, None), (None, None)),
            ("a830cabe22007a168d2f3593bc75cea20b535ea0", (None, None), (None, None)),
            ("8ad112b307c519c866e7eccb709eaeebabf074dc", (None, None), (None, None)),
            ("d4df897a24a5a9cf1d2f8b8c2d4533fb19cc2840", (None, None), (None, None)),
            ("c11e837143228d912f22f0753b10ea8fa322a7b0", (None, None), (None, None)),
            ("9550222fdf1a7caf62f818a2fbd6bf1e5006dcb2", (None, None), (None, None)),
            ("e8d2a6b110ddd85424de375697ce615fc338e21e", (None, None), (None, None)),
            ("fcd401d345d3951ba8515128096a63ae9b84b827", (None, None), (None, None)),
        ],
    ),
    (
        "projects/git_walk/solana",
        "master",
        [
            ("874fbcb9d46d3b748c05d31e46b55b46ff5339cd", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("87b4dc64e3fc8026515e4186c92ff4325e2ab5a7", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("6d2c14e147e0aa420fff39ad29209444885b8ce1", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("a3e4c96bc019dfe87e81534b0be3a4279fdba21f", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("63bd0cdd5d534444ac65d97ea8373337ea0c827c", ("--workspace --exclude solana-bpf-loader-program --exclude solana-accounts-cluster-bench --exclude solana-cli", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-accounts-cluster-bench --exclude solana-cli", None)),
            ("b389d509a8ecb8337adf1feca78ff7cfeccb112d", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("169307d40541fe454c7063c07a6db0261fa9937c", ("--workspace --exclude solana-local-cluster", None), ("--workspace --exclude solana-local-cluster", None)),
            ("a2e7d1356c4ec3accee20cab9a4ed6743251cc62", (None, None), (None, None)),
            ("4940d530b8e5d4aa1ee4ae05aee55b44cdde3663", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("cd93719f68a20e78bff9f0104860cdf6d914f049", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None)),
            ("284c41a6db8cc17e75a66c2a7814bb1ef181a56e", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None)),
            ("416b45ac0f69cfad17e1af06f67055e228a585ad", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("8940dd01fe954dafd97837bb3ea001fa78df947d", ("--workspace --exclude solana-local-cluster", None), ("--workspace --exclude solana-local-cluster", None)),
            ("a3395a786aea208f81516c0e43d71c239bc0a631", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("39615bd075cc330177b0ac5bb056ce77a4a107b4", ("--workspace --exclude solana-local-cluster", None), ("--workspace --exclude solana-local-cluster", None)),
            ("14d0759af0663c7739ffd0c6e94c3c57f1ebcff1", (None, None), (None, None)),
            ("1693af8e686f582c81ab21ca32c31fe3e8e267ee", ("--workspace --exclude solana-local-cluster", None), ("--workspace --exclude solana-local-cluster", None)),
            ("37887d487ce9b87297d9a20dd07a89adf4c66cd5", (None, None), (None, None)),
            ("7c8b846344e2bf776caa008d064fe70f58e9b5b6", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None)),
            ("e3e888c0e0d24e61c9fb9a1a52e054e4fa57a11d", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("4f947a0db3ff9a1622fc50527134ea7617860da8", ("--workspace --exclude solana-bpf-loader-program --exclude solana-accounts-cluster-bench --exclude solana-cli", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-accounts-cluster-bench --exclude solana-cli", None)),
            ("a5f290a66f10a25e0c3ee004a1fcd2889eca2785", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("052677595c4314d2d6e9a258c2556393575cf70c", ("--workspace --exclude solana-local-cluster", None), ("--workspace --exclude solana-local-cluster", None)),
            ("c65a8ce6c3fd0501da7bae06dcbfcbfcd0e39623", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps --exclued solana-local-cluster", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps --exclued solana-local-cluster", None)),
            ("a4b8ab2f590ca4d257e9c00e5320ce1aa1732177", ("--workspace --exclude solana-bpf-loader-program", None), ("--workspace --exclude solana-bpf-loader-program", None)),
            ("43668c42462ed97d603eec14b865eccbea950f73", (None, None), (None, None)),
            ("0bae8d8c151a4f47469eb2f954238e5c9a3ba095", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None)),
            ("4ac52c2a9d2bcb0abd015b5ad3c2c87d34b989d2", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None)),
            ("b619b0d33f2a5b525003b4b8f4ad6c4f3a42ceb7", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None)),
            ("42cc76e33dbd6f54033101b016d1d58d25f9943e", ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None), ("--workspace --exclude solana-bpf-loader-program --exclude solana-bench-tps", None)),
        ],
    ),
]

SMALL_HISTORY_PROJECTS = [
    (
        "projects/small/anyhow",
        "master",
        [
            ("cbd91b854f9ccdcf6db1faaaa81fbe353feaa4d0", (None, None), (None, None)),
            ("47a4fbfa365050b293d9e3898aadb42a47a571e6", (None, None), (None, None)),
        ],
    )
]

SMALL_MUTANTS_PROJECTS = [
    (
        "projects/small/budget",
        "master",
        [("2db4b033e5fc9ba05010def0f6988ba9b822ae8e", (None, None), (None, None))],
    ),
]


########################################################################################################################
# Commands


@click.group(name="evaluate")
@click.argument("url", type=str)
@click.pass_context
def evaluate(ctx, url: str):
    """
    Run parts of the evaluation.

    Arguments:

        URL is the database connection string of the format dialect[+driver]://user:password@host/dbname[?key=value..].

    Examples:

        $ rts_eval evaluate postgresql://user:pass@localhost:5432/db mutants full
    """
    # set options
    echo = "debug" if ctx.obj["debug"] else False

    # create db connection
    try:
        spinner = start_spinner("Connecting to database {}".format(url))
        conn = DBConnection(url, echo=echo)
        spinner.stop()
        click_echo_success(SUCCESS_CONNECTED_MSG)
        ctx.obj["connection"] = conn
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_CONNECTED_MSG)
        raise e


@evaluate.command(name="mutants")
@click.argument("mode", type=click.Choice(["full", "small"]), required=True)
@click.pass_obj
def mutants(ctx, mode):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Running mutants evaluation...")

        if mode == "small":
            projects = SMALL_MUTANTS_PROJECTS
        else:
            projects = MUTANTS_PROJECTS

        for path, branch, commits in projects:
            spinner.info("Evaluating project in " + path)

            mutants_walk.walk(conn, os.path.abspath(path), branch=branch, commits=commits)

        spinner.stop()
        click_echo_success(SUCCESS_MUTANTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MUTANTS_MSG)
        raise e


@evaluate.command(name="history")
@click.argument("mode", type=click.Choice(["random", "hardcoded", "small"]), required=True)
@click.argument("strategy", type=click.Choice(["sequential", "parallel"]), required=True)
@click.pass_obj
def history(ctx, mode, strategy):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Running git walk testing evaluation...")

        sequentially = strategy == "sequential"

        if mode == "small":
            projects = SMALL_HISTORY_PROJECTS
        else:
            projects = TESTING_PROJECTS

        for path, branch, commits in projects:
            spinner.info("Evaluating project in " + path)

            if mode == "random":
                history_walk.walk(conn, path, branch=branch, commits=None, sequentially=sequentially)
            else:
                history_walk.walk(
                    conn,
                    path,
                    branch=branch,
                    commits=commits,
                    sequentially=sequentially,
                )

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e


@evaluate.command(name="sample")
@click.argument("path", type=str, required=True)
@click.argument("branch", type=str, default="main")
@click.pass_obj
def history(ctx, path, branch):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Running git walk testing sampling...")

        history_walk.walk(
            conn,
            path,
            branch=branch,
            commits=None,
        )

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e
