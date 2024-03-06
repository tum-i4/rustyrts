import os
from pathlib import Path

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
        "projects/mutants/tracing",  # 4.5G
        "master",
        [("3de7f8c6016aebc22228375dc9100c02e955c6d4", None, None)],
    ),
    (
        "projects/mutants/tabled",  # 2.0G
        "master",
        [("cc4a110d5963b7eede0e634c83c44d9e8b8250e4", None, None)],
    ),
    (
        "projects/mutants/regex",  # 1.1G
        "master",
        [("b5ef0ec281220d9047fed199ed48c29af9749570", None, None)],
    ),
    (
        "projects/mutants/rustls",  # 709M
        "main",
        [("45197b807cf0699c842fcb85eb8eca555c74cc04", None, None)],
    ),
    (
        "projects/mutants/rust-openssl",  # 559M
        "master",
        [("cc2850fff7c4b0d50a23e09059b0040044dd9616", None, None)],
    ),
    (
        "projects/mutants/pulldown-cmark",  # 507M
        "master",
        [("967dd38554399573279855a9e124dc598a0e3200", None, None)],
    ),
    (
        "projects/mutants/orion",  # 472M
        "master",
        [("cfa2c0c1e89f1ec3d2ab1ab1d57f88c1201e452c", None, None)],
    ),
    (
        "projects/mutants/ripgrep",  # 441M
        "master",
        [("af6b6c543b224d348a8876f0c06245d9ea7929c5", None, None)],
    ),
    (
        "projects/mutants/rust-brotli",  # 273M
        "master",
        [("b1f5aed58287cb01795a099230faa7d2ac734740", None, None)],
    ),
]

########################################################################################################################
# Projects configuration for testing evaluation


TESTING_PROJECTS = [
    (
        "projects/git_walk/actix-web",
        "master",
        [
            ("f5fd6bc49fd0886cf4a1c76de44c259aff7426c9", None, None),
            ("55a2a59906c05f834fe1278fafb8a8dd5c746510", None, None),
            ("3fde3be3d863a42fab6799bdfd4552bcb43d7e8e", None, None),
            ("987067698b11db82f113ae350356cb11d1d7d1f0", None, None),
            ("c659c33919c4880dbe3d220773f20fc6c5b58070", None, None),
            ("4431c8da654f141f564c9715e4d2962d48e0ed69", None, None),
            ("1a0bf32ec76411e6ae017ea680b4dad7db3f0c69", None, None),
            ("d0b5fb18d2f16f42743518363be8b9e0737cee56", None, None),
            ("0b5b463cfa951d96ec6b0167964ef613b0d2b091", None, None),
            ("679f61cf3751ce9ca53ab64b01baef33b83db937", None, None),
            ("c313c003a4b8b3526b33f782996116263cba7140", None, None),
            ("c9fdcc596db0618495ab8611e52b730e829e36e5", None, None),
            ("e4503046de1263148e1b56394144b1828bbfdac0", None, None),
            ("fa78da81569f23accfe7b293be0c022fd01bbdb3", None, None),
            ("7030bf5fe82dc3c23e61a6f0e7a6b89b53a03e6d", None, None),
            ("1716380f0890a1e936d84181effeb63906c1e609", None, None),
            ("0f09415469843eea4000dc48085101dcf8d75e9b", None, None),
            ("f45038bbfe338661f3b958b10c37dd64d3d70650", None, None),
            ("9aab382ea89395fcc627c5375ddd8721cc47c514", None, None),
            ("21a08ca7969e9a08035a4b9e78d8419f3cce3c64", None, None),
            ("df6fde883c17c39ee30b71858a65c36eb0ed71c0", None, None),
            ("162121bf8d51d497565961d73dde22ba1c36f3a4", None, None),
            ("a35804b89f5b08b11304d4fa3e4ca37c9a4f6627", None, None),
            ("a2d4ff157ea981a09d56e4284fd484e88a5c498d", None, None),
            ("2a5215c1d6cce12ff3f4bdc4e7ac73190d4aa9e0", None, None),
            ("679d1cd51360f62fe5f0084893591b6003671091", None, None),
            ("c22a3a71f2b366bf7af6fd0e00e5f150835645c0", None, None),
            ("0bc4ae9158d51e06e4845bc2e0ba987a5e3aae2e", None, None),
            ("de815dd99cfd95e2759080a7d27535b70b6544b4", None, None),
            ("35006e9cae904ac28ffe81e62ac2e0110346121d", None, None),
        ],
    ),
    (
        "projects/git_walk/arrow-datafusion",
        "main",
        [
            ("844bcda2664a04685b865afe7ff159c0648d2860", None, None),
            ("0e416f0e39d8a76ea13aab9b8f306b64d1133857", None, None),
            ("dde6f71202abdb41964609e5e699e80885fbc161", None, None),
            ("379f42ca444d280113a4d7b0d6fe6877c122e740", None, None),
            ("91352518dd05af9491f3fecaad5c9077072626cf", None, None),
            ("3da59e8877620a095efae102b0177e7e6a76f65b", None, None),
            ("894be6719373be85fa777028fe3ec534536660e3", None, None),
            ("b6fb0dd52c2abd0f8e134aa46cc1571cc6a0971b", None, None),
            ("cbb05177bce90618021c51622989b58edd882275", None, None),
            ("c2972a29c3165dc8cb92f8d437de80d56d99740e", None, None),
            ("acb245a9c1d4cf30a484a26152ba0489886a8474", None, None),
            ("a25529166f82e4f52d56eba6448a5a32e126698e", None, None),
            ("533e2b4c72da854f4d9e4a9e2d3d7c35814b7ffd", None, None),
            ("1152c14f5a6bb6fa949d5162380f47aeeeca6696", None, None),
            ("cb842b55705d95b6c1367fb13c223f53d019ca5c", None, None),
            ("f37dd7cd231a5423e19c519f66d6b6cffb60d230", None, None),
            ("a6e93a10ab2659500eb4f838b7b53f138e545be3", None, None),
            ("de314e74fb9b423b067fd83976836235effdcb26", None, None),
            ("748b6a65a5fa801595fd80a3c7b2728be3c9cdaa", None, None),
            ("6e0bb8476d783c1caaf6bf011487c92ae9352f78", None, None),
            ("aed3aa46d98ed86c296d447ef4de676ea77c62a2", None, None),
            ("a690a28ec7d35e994c2e021c62c336a4af84209e", None, None),
            ("f100f3bf6d397fcee24a6bde526888f45d2fd058", None, None),
            ("3c1c188e1476575f113a511789e398fdd5c009cd", None, None),
            ("3a313c9d2bb04f104dcd3664851bb22f6b130cd8", None, None),
            ("e5ae1ead6812e36245e379848a45b0078a6b1235", None, None),
            ("9ea7dc6036a7b1d28c7450db4f26720b732a50de", None, None),
            ("43575e1e4bdeb23320ed7a1119a4a6a5e192254b", None, None),
            ("fdb8fecf0ab475ba07dc0d15f7b53e25ccf30ee7", None, None),
            ("2a15e3f13d52ff2c625742810d096608986de0f2", None, None),
        ],
    ),
    (
        "projects/git_walk/exonum",
        "master",
        [
            ("cb1a0384651bd31a869ab1aa65e1806a53a3d7bb", None, None),
            ("78fe27b23209beeb8f55ad2308835de730f03f38", None, None),
            ("3bc48f285343b379738465ff7ae05efb5730b908", None, None),
            ("a295f8744fa4d7511f0058766ef7cbd670cb1d84", None, None),
            ("c36483f7af6c2e1626e48cb83f285310d7c2fa59", None, None),
            ("262ece75ac53dd39cce23d427c03089f30a4c60b", None, None),
            ("8f25d7822fd232c691747ae512f13da92f100ea3", None, None),
            ("d01f37122b6dbca31e37b3422777f2f442160ed9", None, None),
            ("53898fc17b2c1b20e4761d45194d54560c25633d", None, None),
            ("28e8d8393005b2a9cda728168dc1963ab7c7ff4f", None, None),
            ("65b576c311a3140a2f54f54d0853b54d282618ff", None, None),
            ("f5a785aa5349ee30375ff51940a930c2e1cf4ed0", None, None),
            ("7d73d1516935d7710dfec13d1e2ea1884a38ebb8", None, None),
            ("59d7557dc36479bb60edadd6159baa9327b0b2f3", None, None),
            ("3c93db7e6feceadb17f3f143c0ed856db2958ac0", None, None),
            ("882e23d65c916429c19e85b91ee9b5826908a71d", None, None),
            ("29f596b404a163e2f2414b6e19360600e89ce506", None, None),
            ("b8e4797aab905cd389bb46d4d7238d721d80cdb8", None, None),
            ("b614e41feecd3b6d775c492a3e56f49155b908ea", None, None),
            ("87280613d8a66e74f1a7b00ba09f14cccb7e8dd5", None, None),
            ("e75b31a73cf9f09139c77c920c3638eeecdcba2c", None, None),
            ("bdbfb4abdf63dbc0c5fd9f627dccde5c4af5ffef", None, None),
            ("05b435e5d930d8d05ad01839a6241fb62dc3950d", None, None),
            ("47d43c73af945923cd22dd8b989a64d5178a86bd", None, None),
            ("b7618adeecefdc774d0581b5fca180b1e4d7f7dc", None, None),
            ("334a37e39369a2aa3d2922f2d03d6f0c24d175d2", None, None),
            ("38710ad2f37a8d1f8067c1b6186604a33131f742", None, None),
            ("517b82efcd2788531183858cca8d4abb3edbadf3", None, None),
            ("57109131297cb926cb652910c1e0228effc16b58", None, None),
            ("fbc3d17a0993a7cfe8dd7addb4609a551eb53148", None, None),
        ],
    ),
    (
        "projects/git_walk/feroxbuster",
        "main",
        [
            ("216e0e6595fbf9329656983b047f2b2b7d78977d", None, None),
            ("f6047e98197a77e70ce4f599bfaffd35b9e0c67c", None, None),
            ("36994d208df230b0238342ba56005dd7c3d8598d", None, None),
            ("3de31f0393bd46944031560e9c772e20c12714ea", None, None),
            ("bb4a335299e91972a644612d81b37eb50b25938a", None, None),
            ("c23850208b9e4f0d74af77f7762856fbd2fd5443", None, None),
            ("7f145f11df685c2a177947d91bdb0ca62b639b33", None, None),
            ("53281c0921a57d7adadb00596bf0828ccb07a3e4", None, None),
            ("65967591322d3abfe5f135d1dcb0097d074f87ce", None, None),
            ("acf16c92cd7756aaf9afa066344018caf42568cd", None, None),
            ("329d04252f9bcb302bdd5508cd14ee72df75b909", None, None),
            ("5578e8db5c16b3d07b05bbeedf54a2b917b806ec", None, None),
            ("fddade6a1173795e3cb60987d33670e99e3e5f71", None, None),
            ("43e5ad14c9a543e920659a218d92587a71111f01", None, None),
            ("f173147352978d3e7a0ab65ac620d7a480d1d593", None, None),
            ("d36379ba1bafff25e12bff2c6293f83bb6bc50ff", None, None),
            ("fe5612ce71558b826a9f1f6535de309dea117db2", None, None),
            ("6c3e41fc3d7dd676e472c343b4f17208e2e92204", None, None),
            ("3f5ff1ad3e4847f5bd5e525c82db511444a37b50", None, None),
            ("28ecd60f61c52c2960b4505b8e05a5a7590bc96a", None, None),
            ("eddab0de13014e97cd68f708b7f5c6eb6f452d37", None, None),
            ("4f557511b4974ee51ccdaee788907ac1edbc9674", None, None),
            ("c99f6146e34db48163a584658892892398473662", None, None),
            ("4debe68ed67539d146101cd9c4acb3e85d49c42f", None, None),
            ("3a128df2fc416e7b3a7f3dc827e4dfa7dfc6ef0a", None, None),
            ("f3b2193b2f536bd0674ba2d085f39a3ffa5631a2", None, None),
            ("9b1a24bca3e0e1ec1d1310b0881ccb852de4b0df", None, None),
            ("b456215a00816fb4372c1a260b76ed0fca175a0b", None, None),
            ("7f0dcb6b46ac00c27dbff4913548f2d70b7ba8a0", None, None),
            ("82f8f687fd18f46b21e81f31ea189dc9fe29a1cc", None, None),
        ],
    ),
    (
        "projects/git_walk/meilisearch",
        "main",
        [
            ("b99ef3d33644b24cccf9ff167f698b30b99625d1", None, None),
            ("953b2ec4380736136c015ccbf4d508acdf7fc21a", None, None),
            ("79fc3bb84e3659797fc3ca962854fc026e7b6684", None, None),
            ("2b944ecd893c3421baf529ffa060aecb74e5a88f", None, None),
            ("44f231d41ea1ebd349240413d4d38a05ea961d79", None, None),
            ("c09e610bb5e921ff03b2b469c6691ba9be2d231f", None, None),
            ("2ef58ccce9a5016f8563f4bfd8c73c5dda322582", None, None),
            ("0990e95830a43dd7b0b4f513f86f442efedc42d5", None, None),
            ("6af769af20039d258126f2983b36d0145a2cb38d", None, None),
            ("80f7d873563e7f06acde532d4345f4a866390a14", None, None),
            ("8dd79426565660e211431e43ff38e667b502092c", None, None),
            ("f892d122de2f6d41c7228cc25be270fa0b689041", None, None),
            ("ac2af4354d9d2c55d87199e7ea9928d4c1d8c574", None, None),
            ("65ca80bdde170fba76a2d3175b1580ec0654c3cb", None, None),
            ("da04edff8c6b032d70fe58500c530d8b44cd04e1", None, None),
            ("2f3a439566a3257c6722d6b3eca367e84e559f1e", None, None),
            ("cb71b714d71b025f0b08b9408191c22cb6700798", None, None),
            ("9b660e1058eabe31ff1ebde64043f6970afee851", None, None),
            ("d263f762bf88d7e2209d0d849578fd7463606eb4", None, None),
            ("eab1156f8cee2d417ca5242578bc10ca387938ad", None, None),
            ("2830853665a20f751389fb109802bbb1609348fc", None, None),
            ("348d1123882b1c3433d43b179a30d1cf1457fda9", None, None),
            ("5ac129bfa161a0aceda88382ace169c115c43f32", None, None),
            ("b3f60ee8057f837a5de9107db42faa1cfd4fde17", None, None),
            ("d1b364292322028d4ea4fea820b70c13a3bac222", None, None),
            ("6a742ee62c250a0108ef6706c9c52b462b3a2c8f", None, None),
            ("11f47249578871c7f61f3b2fa9c2184c635d2f77", None, None),
            ("05ffe24d645d3b6dce270b94800a2b17ab024fbf", None, None),
            ("cb1d184904e474d178d0041bfe6b9ded0638b0b6", None, None),
            ("c9246145274634f3df1b17bd578dae7eb6f94ddd", None, None),
        ],
    ),
    (
        "projects/git_walk/nushell",
        "main",
        [
            ("5f48452e3b8da8dd76ddb9adf03e832a3041dac9", None, None),
            ("367f79cb4fc2f281abe608840733f78f664b34a7", None, None),
            ("d8cde2ae8958b4c07f9d16a7540fa679cf3b17c0", None, None),
            ("b8b2737890736a94506a07a504c3c9e029b66498", None, None),
            ("57761149f473067e070dae4b42708372b99c0ff0", None, None),
            ("1086fbe9b5ef588fb6fda5f09133fb72fc2e2fb7", None, None),
            ("beec6588722c96928b58a8bc300f4353220493c1", None, None),
            ("56ce10347e29544a1d0ef2ae70ca6acb4d81a573", None, None),
            ("9259a56a28f1dd3a4b720ad815aa19c6eaf6adce", None, None),
            ("0d031636a9ed1602623b0b2b7468935cbf7f3148", None, None),
            ("68d98fcf24e84dae83fa034b7f38779496c93291", None, None),
            ("e97ba9b74c607b1b23f4bda84a90303bc66b3bed", None, None),
            ("0450cc25e0272c3d1147afc27176daf43e52e83b", None, None),
            ("525ed7653f641b55093e863f3dcd84c6fffdf52f", None, None),
            ("f818193b5333eb44da92ce258059906507bd13dc", None, None),
            ("3db0aed9f739fd929f8dff287fafd60883649c07", None, None),
            ("d885258dc781807e5758ce274eb99c2437c75667", None, None),
            ("7490392eb982fbbc1036bf9356ada84764f601b1", None, None),
            ("266fac910a2a23e309b3c61342aeee9c5ce520aa", None, None),
            ("bd6c550470e11e3346cd6d99eeea9bd1a3b8b7f9", None, None),
            ("83458510a922fa6e52761897f5e2a95b763143ef", None, None),
            ("f93033c20baab05f2a0640cbdf5da644349e4976", None, None),
            ("fb42c94b791c1e89c69daaa11ca467f774d16812", None, None),
            ("2bb367f570502cd26d6b7a81425d9f3b3b9eb432", None, None),
            ("909b7d2160ffbc4289b82c8090a196cb136ec712", None, None),
            ("626b1b99cd667243f4587d83bdef17d1e907b106", None, None),
            ("27d798270b2051e850f1b2ee3fa118d9992b8ea4", None, None),
            ("9009f68e0980120d1a7d07e0ecf9bb637a3a9dcc", None, None),
            ("b907bf355fd921f22c0cd1f4e0677ba7c6814b3b", None, None),
            ("9da2e142b2b39c8a8dc659f461f37661c871dcfa", None, None),
        ],
    ),
    (
        "projects/git_walk/rayon",
        "master",
        [
            ("f0c5a47fca1f2a1ba627aa9ed0514eb0855147c4", None, None),
            ("b98bb23f0595cbf56afba8f641b366a0433f002e", None, None),
            ("16b3310e30e4d8af11ccd0d76b3e319816b892d3", None, None),
            ("2d62dc6b89895fb28210acf0e160669654b970d1", None, None),
            ("e20b4ede19accb0d65494016fa5bfcd46b636e5d", None, None),
            ("b3e6600ca34054d8cefa61fbee4245d1dccd6ba5", None, None),
            ("ed6a5f75c4dc1cb0ac4f59f6f01b38f8a695a777", None, None),
            ("545feac3b8bbe7c0da42cb8a3aa60c17a9b16829", None, None),
            ("edaf85171b8acbe968586b676a309cd46225c472", None, None),
            ("e7ed9a857e23e8b216c97adc54a52830183df40c", None, None),
            ("a7f5c779761b34293bcc4c16e420c4c5fc4a1c57", None, None),
            ("35c032c6714d6fd9876976c5ad6793f8052d7ff0", None, None),
            ("aa6fdd3f1b1b2411f546df16fff9f805e2bfa93a", None, None),
            ("33790dc7eb68a114feacf00547bd60a3c5ad3c3d", None, None),
            ("d2bef7e2d8524011f01b06d79711a11b99b9316e", None, None),
            ("b56c6b8fe89b474cf07380730ea4bb660406a3a6", None, None),
            ("d31d3e3b8ee85db761f283cca25de43a7b7931be", None, None),
            ("d466fd04a75325f7e6a37b1f142181dbb1e20c12", None, None),
            ("4fd13b033424be5eac826571e017b1a008d0bd06", None, None),
            ("a0efb4abb5bc3a2ee0ed812392a2b7386dc2dd38", None, None),
            ("f7d75532fcbc8151b97dec85131e5a0be3db4b4f", None, None),
            ("54758597379d34108d55343cdb4a5f390f7e2ec4", None, None),
            ("faf347a416e8998ace15e795d0af9040cd0ec15e", None, None),
            ("51f9676ac4a743d124ed88878d611a8b475f488f", None, None),
            ("dd91c279709dfbaead9f70b28854ce632d040a0f", None, None),
            ("d41d29ab56d08c9c7836fcacf415e457488b574f", None, None),
            ("a116575410ce3e2e003f10c64c565e85ac5b3893", None, None),
            ("c396338d8c3ebec81dff95965d89673b8cdfab9b", None, None),
            ("08345e14b0cca55fcc9a427ef6ef057e65ab6dee", None, None),
            ("073b79b22f23dc78769521555388d18897b48b26", None, None),
        ],
    ),
    (
        "projects/git_walk/rust-analyzer",
        "master",
        [
            ("04decd5e6b36574ca30369c26481d5a51d739971", None, None),
            ("9a481d1ecf0f11b4a5bd0220eec3fd93c997b033", None, None),
            ("572f1c08b6ba43bdd57c5cb99f79a08ecd821c1c", None, None),
            ("f55be75a17dab2ca23b34c45e7597fe19a5fc8e4", None, None),
            ("3e1e6227ca525f8631e0bff2215fa3de1b4f4cc1", None, None),
            ("12d0970f7e4c4d7f91cccb12525fceea3c4c0669", None, None),
            ("9153e96e88236e2f867dee8f0f291af5cfaf90f4", None, None),
            ("095843119e703a756047dfe25514fdcc93425341", None, None),
            ("6746a08b442d25dc63a90ace1682ebd9ec9b50b8", None, None),
            ("8ed8e4f25abc95d06487c34e0b2b85778aa6a4b4", None, None),
            ("9a4553b833e8fa57a361d934c3498efa485d27c9", None, None),
            ("d96c489120551d36f5bdb362ce313052350821f7", None, None),
            ("461c0cc07af36fc95bafa6d5a8a9d86735fc64ff", None, None),
            ("78d6b88f211cc9faf88815ce7fb1a91546cfce15", None, None),
            ("de7662c852353febce09196199202ee7f6e8e6c3", None, None),
            ("59bd6e2eea151f097a65f2634dc5488b3c272d92", None, None),
            ("43cad21623bc5de59598a565097be9c7d8642818", None, None),
            ("21359c3ab5fc497d11b2c0f0435c7635336a726e", None, None),
            ("81847524702dd7cb1eeae25a53444b325295b129", None, None),
            ("364162f8759a407a06b360e383de5404875e6000", None, None),
            ("d2fd252f9de23d5801b1ca10c067654bf7d6ef4f", None, None),
            ("046ae1d361d8941a664919e7668a65ae735d4a1b", None, None),
            ("30b7e92afaa6dc6b276d60b8e7b47485ca7c2ee3", None, None),
            ("cdd7118cbf23e21c376092b3b2734407004b8dbf", None, None),
            ("0448b7364666ba59b39bbd5564fe8a34b67b8f01", None, None),
            ("32f5276465266522ebc01b8417feeba99bf00f6f", None, None),
            ("3cd57c425a1f7001cc86222f928f53a7114564df", None, None),
            ("a6a052f4078825ba307843df1770c92d96827075", None, None),
            ("a1a7b07ad33b7dcadedc2af26c3a5f8ef3daca27", None, None),
            ("91bbc55eedbc0f6947b69a0158a7b6c81264024e", None, None),
        ],
    ),
    (
        "projects/git_walk/tantivy",
        "main",
        [
            ("f51801265640a8144655851072b6468e842f935c", None, None),
            ("c0f524e1a3f49f5609dfc34730959ac57feeaa49", None, None),
            ("2e639cebf89a185cd22e7b5aed25472b7a85b01b", None, None),
            ("36528c5e83e551a19f77e4e141fd09787c00d163", None, None),
            ("e2aa5af0759ff9077bb4490c7601c08d8e70c436", None, None),
            ("111f25a8f7c20abf90e2e6bc1cdd283dcab59b38", None, None),
            ("bc0eb813ffad75059a5e28ec46e2846371c75c06", None, None),
            ("bbb058d9769ca1bd8e4022488486de8410c39338", None, None),
            ("057211c3d8213ad0977e1d6e6f67e08f22cee332", None, None),
            ("bbc0a2e2331387b463067228e2cb6f6df6e78c4f", None, None),
            ("6bf4fee1baaec3d83b456d83d78f507f837a7bd8", None, None),
            ("4583fa270b24e812c37a22e7100a0c6d110f1d0a", None, None),
            ("70f160b3296e0dec38890e712f78f6d859a886f6", None, None),
            ("92f20bc5a23f004ec8d97c8a7bebeea408495ad6", None, None),
            ("784717749f5f129c71031391bb97234c408f677d", None, None),
            ("46d5de920dd1ac86fa7a74baa0debd933bcb6574", None, None),
            ("075c23eb8cc9d9fbfb3e47e2559189b66d980143", None, None),
            ("972cb6c26d002b2f83548855fe8cca0b98bf93d3", None, None),
            ("64bce340b2b32408be7c16cf77eddf5009542965", None, None),
            ("a550c85369f516edb8905e79f68b10637bf38894", None, None),
            ("762e662bfd2c7224a8e30e18fbe61f5b31ee9f42", None, None),
            ("6800fdec9db02a3bf8cdece5c98bfc0650bf415e", None, None),
            ("537021e12d38bcc3d2e5f33ee1d54ced66d057ed", None, None),
            ("46b86a797651e1d09dc9859ee4db705272e6cfa7", None, None),
            ("d31f04587258b47da704a945bbc41d10a759c465", None, None),
            ("62052bcc2d35364ce45ac837ea1b7cea2c106ea5", None, None),
            ("54decc60bb91077ad869eafa31bf3e6b1b22e655", None, None),
            ("cf02e3257836f6c6ae985db80ad5b03c84d147f0", None, None),
            ("443aa1732905264161b953c33cb944be2f5142e0", None, None),
            ("4d634d61ffecf238249cc556a39b6a88701ad6e3", None, None),
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
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
            ),
            (
                "6f2957b4b364c0c664d66e40588282ad5e95a785",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "44339c18fe782c19ad25a2ae63bd11e0cb4fe011",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "c82fd9377d3fc90bb2876e346d407fffe29a2cfc",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "c91e545e03d1a24b083d3ea7368ea0d160e45a4b",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
            ),
            (
                "94374e4e98e402052c60a97a983336183c811146",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "499eb499f01f9870bb1bbd8e768d6a6b6a020a2f",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "8074253fe24c62a3e6bbd9ea8e808409eaf9bf24",
                "test-singlepass,test-universal,coverage",
                "test-singlepass,test-universal,coverage",
            ),
            (
                "9353d09cfabd8a297f3184ec9a5a3787b539c607",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "36419ec7e48a9efecac336bd9cf4b120bc5e13f1",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "03b34144bff602c549dcfbee52520d33252d0e04",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "bb6f483fac97dbad4a85286970c78809cba4abc3",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
            ),
            (
                "a91d2474619388f72b682196ccf70d75289e1745",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "54b4495b3f9a9e3dd60cc1bf00d20d07eb777bab",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
            ),
            (
                "df30d25d3952ce4b04a75c4353fe8f90df987b1e",
                "test-singlepass,test-universal,coverage",
                "test-singlepass,test-universal,coverage",
            ),
            (
                "66bfb88d8374be4ffdbfae3257bfc30c6bde0c1f",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "9e83e8a4663e4f5f8e95a8d823e96b8a6390ec50",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "19c4c623a819f3f3bac28af91d749254fa82fcfc",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "f8c0910c337a0e571abcf457f42e936aa98e275e",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "9ecedc3925f7c764916025c78bdc626cf45ef46b",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
                "test-singlepass,test-cranelift,test-dylib,test-universal,coverage",
            ),
            (
                "c03d61b78a1a0bbd5f542cf1235e88b6a639ee6a",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "59d065ee32a24e8f82e6c762c4d6ecaa3a481879",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "88b4093646f60e680dc3fa3be9877c2ad77257a0",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "a869daea916863cb63e6fcebe955417aebd3b5b4",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "156f0483b3c7868bcf3fb56f84b1c35cd8eb9531",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "9ae02d5c0b66bcaff799ad4c64ab70057bc01e09",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "1dad3dd1cb6b94b0197938134839cf8fe9fbe9a5",
                "test-singlepass,test-universal,coverage",
                "test-singlepass,test-universal,coverage",
            ),
            (
                "3d6241236c50bf4f8f378c50abeb84b733116a00",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
            (
                "1dd6d8d3eb4f2633e1f05acce50803302f8a3498",
                "test-singlepass,test-jit,coverage",
                "test-singlepass,test-jit,coverage",
            ),
            (
                "59b37e4b75a5b0d5cd701858defbfbc99dbace13",
                "test-singlepass,test-cranelift,test-universal,coverage",
                "test-singlepass,test-cranelift,test-universal,coverage",
            ),
        ],
    ),
    (
        "projects/git_walk/zenoh",
        "master",
        [
            ("431ffd48f6dd4192f63a168efbd84bcfa7142749", None, None),
            ("472cc34acb9d85f781a1c6fe735fa124c88ce97b", None, None),
            ("0e791bc11d6819f68d143072f7bf1bb60b341ebf", None, None),
            ("f7ff3f4278b96ef60eb14f8907c36d5cc2795dee", None, None),
            ("7ec9427f8cb091387802d6a7e1dd910248941ef8", None, None),
            ("974441cd7addd8bb22ed7a819652fee9115b9d1e", None, None),
            ("85409a54adb7598bf5d07b95ccc4d01a2f0331e3", None, None),
            ("e144f3d502ac69f9ab2a9b612525a7f67b3f2a1e", None, None),
            ("eab33b2a9f6a136f5a2b9823358d58690741b0be", None, None),
            ("26c77ce77dee5f850358df3a696c45690cdfe5e9", None, None),
            ("8b78e6a090cad12a8f0271667025085538cfb389", None, None),
            ("3d94ac1a22268fa06ee0c76cdd385ba1ce454a4a", None, None),
            ("3302c23929e4e66f5aced4df5e03b4eafd8b2f3f", None, None),
            ("2a806883239b1a377e5012b610973562c1127f6e", None, None),
            ("65fed69e8500e066375568f69d1616779a83e509", None, None),
            ("38f7334a9e8fac86f631b254d5e64a5053e5b671", None, None),
            ("0cb17cd9017eb9555819ad9c3d65ccfb10feee0a", None, None),
            ("059c5bf3cfca916a3dbaa7d4ca752edff9a3205d", None, None),
            ("59d6b90306c377982e45c2941b935a5efddf436a", None, None),
            ("1617651e0a961bfa406f3deb58647906b34c02fc", None, None),
            ("91eba62fe07fbabfe2a0421ba57caa20e7e88db6", None, None),
            ("9405b41e8756c3652837b3f40d3075c6b7ee9bdc", None, None),
            ("bd237c2e3efbe690f61df1126b7e016a216f6cbd", None, None),
            ("ba371c8a76755c64379ed144ec55311775141d1e", None, None),
            ("14dcff357dd57410dab339c9b0db9a3d5dff8c0d", None, None),
            ("a8b6de0d27b384ee0f48154b2a3b93c0acc2b7e6", None, None),
            ("89b88cbe40e4d40b8c2351cd903ef127395ceae8", None, None),
            ("5bd713eb8ed1ea4d68010c1c4f7d2dcd2a95922b", None, None),
            ("3fcace64b931531fd44385d9b491909586033063", None, None),
            ("6f98a5ee336f98cb592838451c382b154499d0c4", None, None),
        ],
    ),
    (
        "projects/git_walk/penumbra",
        "main",
        [
            (
                "4235275e5033945ad7ad4a2137326cf4d75380ed",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "8cd239964ef0a45c41b36fa2c16e4dd9412eeffd",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "c6b53389f5e0968aa08b1c849f95d0438c6a8f70",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "9f04762adbd96dbd04c7a36e84cd74568e81f915",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "fc2edd093b94dddc9fbf42a7b409343942681138",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "c2bc2c0fc498dbd7e1134ffea18cc3452286685c",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "4574c1acc26b8f83f264ec46be1ee48fb4ab4010",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "cc784a2143b6ce04c5a164b102aeb5e819542e73",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "ac48951257a3608584fbd6647ea2e26effde67c4",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "b0f80fa28bb69af2a1b0433930b1697fb0666d68",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "89c5b3d42c218ffd8f04aed8e707ca9c8c0edf0f",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "0dfff7b47d840e8d4f66dc1998f87dc9d4ed8670",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "6f504cec09e6900f2818f4c2bf2e5deb249eae7d",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "210d2b9c1cd0d9cd99ec20eeff0114652ac178c1",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "ac6fcef3420905eaed7020fc8226d71a3df8562a",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "46e6a611caf97e11df54e148acde84f23613d6cd",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "66c3f91f43462100c9a83dce85382fd09fee6f26",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "954cdbf58d4bf23f589b4c0edb5d938713ced25c",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "ca7f0ff3b7235d5b8a4334036eabae3cd1705b09",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "b9d325098a1acd260c5efaa094e74e0271161f56",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "686ff689e05c2cabf607760a04fc1d82a8fb584f",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "96189b4567e89fee841d6e327b3a49a5f9870891",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "86f2b9839ee2d1ed4dbb2c6d7217687df6f73125",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "a830cabe22007a168d2f3593bc75cea20b535ea0",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "8ad112b307c519c866e7eccb709eaeebabf074dc",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "d4df897a24a5a9cf1d2f8b8c2d4533fb19cc2840",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "c11e837143228d912f22f0753b10ea8fa322a7b0",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "9550222fdf1a7caf62f818a2fbd6bf1e5006dcb2",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "e8d2a6b110ddd85424de375697ce615fc338e21e",
                "tokio_unstable",
                "tokio_unstable",
            ),
            (
                "fcd401d345d3951ba8515128096a63ae9b84b827",
                "tokio_unstable",
                "tokio_unstable",
            ),
        ],
    ),
]

SMALL_HISTORY_PROJECTS = [
    (
        "projects/small/budget",
        "master",
        [
            ("2db4b033e5fc9ba05010def0f6988ba9b822ae8e", None, None),
            ("701986ccc213eae976fa8f1bd4118132a5a3f005", None, None),
        ],
    )
]

SMALL_MUTANTS_PROJECTS = [
    (
        "projects/small/budget",
        "master",
        [("2db4b033e5fc9ba05010def0f6988ba9b822ae8e", None, None)],
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

            mutants_walk.walk(
                conn, os.path.abspath(path), branch=branch, commits=commits
            )

        spinner.stop()
        click_echo_success(SUCCESS_MUTANTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MUTANTS_MSG)
        raise e


@evaluate.command(name="history")
@click.argument(
    "mode", type=click.Choice(["random", "hardcoded", "small"]), required=True
)
@click.argument(
    "strategy", type=click.Choice(["sequential", "parallel"]), required=True
)
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
                history_walk.walk(
                    conn, path, branch=branch, commits=None, sequentially=sequentially
                )
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
