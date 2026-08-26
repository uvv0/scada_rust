#include "module_programming.h"
#include <QCoreApplication>
#include <QDebug>

int main(int argc, char **argv)
{
    QCoreApplication app(argc, argv);
    QByteArray image;
    ModuleProgramming::ImageInfo info;
    QString error;
    if (!ModuleProgramming::loadImage(
            "D:/picoC/3/Release/Exe/module_slot0.bin", 0,
            &image, &info, &error))
    {
        qCritical() << error;
        return 1;
    }
    const QList<QByteArray> frames =
        ModuleProgramming::makeDataWriteFrames(image);
    if (info.calculatedCrc != 0x66B5 || frames.size() != 17 ||
        frames.first().toHex().left(20) != "f835102710007bf6b566")
    {
        qCritical() << info.calculatedCrc << frames.size()
                    << frames.first().toHex();
        return 2;
    }
    qDebug() << "OK" << image.size() << frames.size()
             << QString::number(info.calculatedCrc, 16);
    return 0;
}
