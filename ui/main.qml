import QtQuick 2.5
import QtQuick.Controls 2.5
import net.asivery.AppLoad 1.0

Rectangle {
    id: root
    anchors.fill: parent
    property var ips: []
    property var ready: false
    property var mainText: ''

    AppLoad {
        id: endpoint
        applicationID: "rmstream"

        onMessageReceived: (type, contents) => {
            if(type == 2) {
                mainText = contents;
                return;
            } else if(type == 1) {
                ready = true;
            } else if(type == 0){
                let toks = contents.split(",");
                ips = toks.slice(1, toks.length);
                ready = toks[0] == '1' || toks[0] == 'true';
            }
            mainText = `The service is hosted on:\n${ips.map(e => '- ' + e).join('\n')}\nThe service is${ready ? '' : ' NOT'} running.`;
        }
    }

    signal close
    function unloading() {
        console.log("We're unloading!");
    }

    Text {
        id: text
        anchors.top: parent.top
        anchors.topMargin: 10
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        text: `RmStream`
        font.pointSize: 48
    }

    Text {
        anchors.top: text.bottom
        anchors.left: parent.left
        anchors.margins: 10
        text: mainText
        font.pointSize: 36
    }

    Rectangle {
        width: 500 * 2 + 20
        height: 80
        anchors.centerIn: parent

        Rectangle {
            width: 500
            height: parent.height
            anchors.left: parent.left
            border.width: 2
            border.color: "black"
            anchors.verticalCenter: parent.verticalCenter
            Text {
                anchors.fill: parent
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                text: "Close window"
                font.pointSize: 24
            }

            MouseArea {
                anchors.fill: parent
                onClicked: () => {
                    root.close();
                }
            }
        }

        Rectangle {
            width: 500
            height: parent.height
            anchors.right: parent.right
            border.width: 2
            border.color: "black"
            anchors.verticalCenter: parent.verticalCenter
            Text {
                anchors.fill: parent
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                text: "Stop streaming"
                font.pointSize: 24
            }


            MouseArea {
                anchors.fill: parent
                onClicked: () => {
                    endpoint.terminate();
                }
            }
        }
    }
}
